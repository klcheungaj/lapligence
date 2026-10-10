#!/usr/bin/env python3
import argparse
import os
import platform
import shutil
import struct
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import ci_build_timings

MACHINES = {
    0x014C: "x86",
    0x01C4: "arm32",
    0x8664: "x64",
    0xA641: "arm64ec",
    0xAA64: "arm64",
}

TOOLS = ("cmake", "ninja", "cl", "link", "lib", "ccache", "python", "bash", "git", "cargo-nextest")

DEFENDER_STATUS = (
    "RealTimeProtectionEnabled",
    "OnAccessProtectionEnabled",
    "BehaviorMonitorEnabled",
    "IoavProtectionEnabled",
    "AntivirusEnabled",
    "IsTamperProtected",
    "AMRunningMode",
)

PROCESS_TRACE = r"""
$null = Register-CimIndicationEvent -ClassName Win32_ProcessStartTrace -SourceIdentifier llgprobe
try {
    & $env:LLG_PROBE_EXE @($env:LLG_PROBE_ARGS -split "`n") | Out-Null
    "llg exit code: $LASTEXITCODE"
} finally {
    Start-Sleep -Milliseconds 2000
    $events = @(Get-Event -SourceIdentifier llgprobe -ErrorAction SilentlyContinue)
    Unregister-Event -SourceIdentifier llgprobe
    "processes started during one fresh model build: $($events.Count)"
    $events | ForEach-Object { $_.SourceEventArgs.NewEvent.ProcessName } | Group-Object |
        Sort-Object Count -Descending | ForEach-Object { "{0,6} {1}" -f $_.Count, $_.Name }
}
"""


def pe_machine(path):
    try:
        with open(path, "rb") as handle:
            header = handle.read(4096)
    except OSError as error:
        return f"unreadable ({error.strerror})"
    if len(header) < 0x40 or header[:2] != b"MZ":
        return "not a PE image"
    offset = struct.unpack_from("<I", header, 0x3C)[0]
    if offset + 6 > len(header) or header[offset:offset + 4] != b"PE\0\0":
        return "not a PE image"
    machine = struct.unpack_from("<H", header, offset + 4)[0]
    return MACHINES.get(machine, f"machine 0x{machine:04x}")


def tool_lines(extra=()):
    lines = [f"host: {platform.system()} {platform.machine()} "
             f"PROCESSOR_ARCHITECTURE={os.environ.get('PROCESSOR_ARCHITECTURE', '-')} "
             f"python={platform.python_implementation()} {platform.python_version()} ({platform.machine()})"]
    candidates = [(name, shutil.which(name)) for name in TOOLS]
    launcher = os.environ.get("LLG_C_LAUNCHER")
    if launcher:
        candidates.append(("LLG_C_LAUNCHER", launcher))
    candidates.extend((Path(path).name, path) for path in extra)
    for name, path in candidates:
        if not path:
            lines.append(f"{name}: not found")
        elif os.name == "nt":
            lines.append(f"{name}: {pe_machine(path)} {path}")
        else:
            lines.append(f"{name}: {path}")
    return lines


def powershell(script):
    exe = shutil.which("powershell") or shutil.which("pwsh")
    if not exe:
        return "powershell not found"
    result = subprocess.run([exe, "-NoProfile", "-NonInteractive", "-Command", script],
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace")
    return result.stdout.strip()


def defender_state():
    fields = ", ".join(DEFENDER_STATUS)
    return powershell(
        f"Get-MpComputerStatus | Format-List {fields}; "
        "$p = Get-MpPreference; "
        "'ExclusionPath: ' + ($p.ExclusionPath -join '; '); "
        "'ExclusionProcess: ' + ($p.ExclusionProcess -join '; '); "
        "'DisableRealtimeMonitoring: ' + $p.DisableRealtimeMonitoring"
    )


def report(args):
    print("\n".join(tool_lines(args.tool)))
    if os.name == "nt":
        print(defender_state())
    return 0


def median_ms(samples):
    ordered = sorted(samples)
    return ordered[len(ordered) // 2] * 1000 if ordered else 0.0


def time_command(command, runs):
    samples = []
    for _ in range(runs):
        started = time.perf_counter()
        try:
            subprocess.run(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        except OSError:
            return None
        samples.append(time.perf_counter() - started)
    return median_ms(samples)


def time_file_writes(directory, count):
    directory.mkdir(parents=True, exist_ok=True)
    payload = b"/* llg probe */\n" * 256
    started = time.perf_counter()
    for index in range(count):
        (directory / f"f{index}.c").write_bytes(payload)
    written = time.perf_counter() - started
    started = time.perf_counter()
    shutil.rmtree(directory, ignore_errors=True)
    removed = time.perf_counter() - started
    return written * 1000 / count, removed * 1000 / count


def find_simulator(out_dir):
    for name in ("sim.exe", "sim"):
        for candidate in sorted(Path(out_dir).rglob(name)):
            if candidate.is_file():
                return candidate
    return None


def llg_command(args, out_dir):
    return [args.llg, "--top", args.top, "--out-dir", str(out_dir), args.source]


def measure(args):
    work = Path(args.work).resolve()
    work.mkdir(parents=True, exist_ok=True)
    cache = Path(args.runtime_cache).resolve() if args.runtime_cache else work / "rc"
    timings_file = work / f"timings-{args.label}.tsv"
    profiles = work / f"profiles-{args.label}"
    env = dict(os.environ)
    env["LLG_RUNTIME_CACHE_DIR"] = str(cache)
    env.pop("LLG_BUILD_TIMINGS", None)
    env.pop("LLG_CMAKE_PROFILE_DIR", None)
    print(f"== {args.label}")
    for name, command in (("cmake --version", ["cmake", "--version"]),
                          ("ninja --version", ["ninja", "--version"]),
                          ("cl (banner)", ["cl"])):
        cost = time_command(command, args.spawns)
        print(f"spawn {name}: " + ("not found" if cost is None else f"median {cost:.1f} ms"))
    per_write, per_remove = time_file_writes(work / f"files-{args.label}", args.files)
    print(f"{args.files} new 4 KiB files: {per_write:.2f} ms each to write, {per_remove:.2f} ms each to remove")
    started = time.perf_counter()
    warm = subprocess.run(llg_command(args, work / f"{args.label}-warm"), env=env,
                          stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace")
    print(f"warm-up build (runtime archive and toolchain seed): exit {warm.returncode}, "
          f"{(time.perf_counter() - started):.1f} s")
    if warm.returncode != 0:
        print(warm.stdout[-4000:])
        return 0
    simulator = find_simulator(work / f"{args.label}-warm")
    if simulator:
        copy = work / f"{args.label}-copy{simulator.suffix}"
        shutil.copy2(simulator, copy)
        runs = [time_command([str(copy)], 1) for _ in range(3)]
        print("new executable, first three runs: " + ", ".join(f"{run:.1f} ms" for run in runs if run is not None))
    env["LLG_BUILD_TIMINGS"] = str(timings_file)
    env["LLG_CMAKE_PROFILE_DIR"] = str(profiles)
    for index in range(args.runs):
        out_dir = work / f"{args.label}-{index}"
        result = subprocess.run(llg_command(args, out_dir), env=env, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT, text=True, errors="replace")
        if result.returncode != 0:
            print(f"build {index} failed with {result.returncode}:\n{result.stdout[-4000:]}")
    if timings_file.is_file():
        records = ci_build_timings.parse(timings_file.read_text(encoding="utf-8", errors="replace").splitlines())
        print(ci_build_timings.summarize(records))
    loaded, unreadable = ci_build_timings.load_profiles(profiles)
    print(ci_build_timings.summarize_profiles(loaded))
    if unreadable:
        print(f"unreadable profiles: {unreadable}")
    if os.name == "nt":
        env["LLG_PROBE_EXE"] = args.llg
        env["LLG_PROBE_ARGS"] = "\n".join(llg_command(args, work / f"{args.label}-traced")[1:])
        exe = shutil.which("powershell") or shutil.which("pwsh")
        if exe:
            traced = subprocess.run([exe, "-NoProfile", "-NonInteractive", "-Command", PROCESS_TRACE], env=env,
                                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace")
            print(traced.stdout.strip())
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    rep = commands.add_parser("report")
    rep.add_argument("--tool", action="append", default=[])
    rep.set_defaults(handler=report)
    mes = commands.add_parser("measure")
    mes.add_argument("--llg", required=True)
    mes.add_argument("--source", required=True)
    mes.add_argument("--top", default="tb")
    mes.add_argument("--work", required=True)
    mes.add_argument("--runtime-cache")
    mes.add_argument("--label", default="probe")
    mes.add_argument("--runs", type=int, default=5)
    mes.add_argument("--spawns", type=int, default=10)
    mes.add_argument("--files", type=int, default=200)
    mes.set_defaults(handler=measure)
    args = parser.parse_args(argv)
    return args.handler(args)


if __name__ == "__main__":
    sys.exit(main())
