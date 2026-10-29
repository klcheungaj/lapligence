#!/usr/bin/env python3
import argparse
import json
import re
import subprocess
import sys
import urllib.error
import urllib.request
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:
    try:
        import tomli as tomllib
    except ModuleNotFoundError:
        tomllib = None

CRATES_INDEX = "https://index.crates.io"
RUST_REPOSITORY = "https://github.com/rust-lang/rust"
NETWORK_TIMEOUT_SECONDS = 30
DEPENDENCY_TABLES = ("dependencies", "dev-dependencies", "build-dependencies")
STABLE_VERSION = re.compile(r"v?(\d+(?:\.\d+)*)")
ACTION_USE = re.compile(r"^\s*-?\s*uses:\s*['\"]?([^'\"\s#]+)", re.MULTILINE)
REPORT_TITLE = "Dependency audit and upgrade reminder"


def parse_version(text):
    match = STABLE_VERSION.fullmatch(text.strip())
    return tuple(int(part) for part in match.group(1).split(".")) if match else None


def format_version(version):
    return ".".join(str(part) for part in version)


def latest_version(names):
    versions = [(parse_version(name), name) for name in names]
    versions = [entry for entry in versions if entry[0] is not None]
    return max(versions)[1] if versions else None


def semver_compatible(locked, latest):
    locked = (tuple(locked) + (0, 0, 0))[:3]
    latest = (tuple(latest) + (0, 0, 0))[:3]
    for index in range(3):
        if locked[index] != 0 or index == 2:
            return locked[:index + 1] == latest[:index + 1]
    return False


def direct_dependencies(manifest):
    tables = [(kind, manifest.get(kind, {})) for kind in DEPENDENCY_TABLES]
    for target, entries in manifest.get("target", {}).items():
        tables.extend((f"{kind} ({target})", entries.get(kind, {})) for kind in DEPENDENCY_TABLES)
    crates = {}
    for kind, entries in tables:
        for key, spec in entries.items():
            if isinstance(spec, dict):
                if "path" in spec or "git" in spec:
                    continue
                key = spec.get("package", key)
            crates.setdefault(key, []).append(kind)
    return crates


def locked_versions(lockfile):
    versions = {}
    for package in lockfile.get("package", []):
        if "source" not in package or not package["source"].startswith("registry+"):
            continue
        version = parse_version(package["version"])
        if version is None:
            continue
        current = versions.get(package["name"])
        if current is None or version > current:
            versions[package["name"]] = version
    return versions


def index_path(name):
    name = name.lower()
    if len(name) <= 2:
        return f"{len(name)}/{name}"
    if len(name) == 3:
        return f"3/{name[0]}/{name}"
    return f"{name[:2]}/{name[2:4]}/{name}"


def latest_index_version(index_text):
    names = []
    for line in index_text.splitlines():
        if line.strip():
            entry = json.loads(line)
            if not entry.get("yanked", False):
                names.append(entry["vers"])
    return latest_version(names)


def outdated_crates(direct, locked, latest):
    rows = []
    for name in sorted(direct):
        if name not in locked or latest.get(name) is None:
            continue
        newest = parse_version(latest[name])
        if newest <= locked[name]:
            continue
        action = "`cargo update`" if semver_compatible(locked[name], newest) else "bump Cargo.toml"
        rows.append((name, format_version(locked[name]), latest[name], action, ", ".join(direct[name])))
    return rows


def audit_findings(audit):
    vulnerabilities = []
    for entry in audit.get("vulnerabilities", {}).get("list", []):
        advisory = entry.get("advisory", {})
        patched = ", ".join(entry.get("versions", {}).get("patched", [])) or "none"
        vulnerabilities.append((advisory.get("id", "?"), entry["package"]["name"], entry["package"]["version"],
                                advisory.get("title", ""), patched))
    warnings = []
    for kind, entries in sorted(audit.get("warnings", {}).items()):
        for entry in entries:
            advisory = entry.get("advisory") or {}
            warnings.append((kind, advisory.get("id", "-"), entry["package"]["name"], entry["package"]["version"],
                             advisory.get("title", "")))
    return vulnerabilities, warnings


def parse_ls_remote(output):
    tags = {}
    for line in output.splitlines():
        if "\t" not in line:
            continue
        commit, ref = line.split("\t", 1)
        if not ref.startswith("refs/tags/"):
            continue
        name = ref[len("refs/tags/"):]
        if name.endswith("^{}"):
            tags[name[:-3]] = commit
        else:
            tags.setdefault(name, commit)
    return tags


def submodule_status(pinned_commit, tags):
    pinned = [name for name, commit in tags.items() if commit == pinned_commit]
    pinned_name = latest_version(pinned) or (pinned[0] if pinned else pinned_commit[:12])
    latest = latest_version(tags)
    pinned_version = parse_version(pinned_name) if pinned else None
    behind = latest is not None and (pinned_version is None or parse_version(latest) > pinned_version)
    return pinned_name, latest, behind


def action_references(texts):
    actions = {}
    for text in texts:
        for use in ACTION_USE.findall(text):
            if use.startswith(("./", "docker://")) or "@" not in use:
                continue
            path, ref = use.split("@", 1)
            if not ref.startswith("v") or parse_version(ref) is None:
                continue
            repository = "/".join(path.split("/")[:2])
            actions.setdefault(repository, set()).add(ref)
    return actions


def outdated_actions(actions, latest):
    rows = []
    for repository in sorted(actions):
        newest = latest.get(repository)
        if newest is None:
            continue
        for ref in sorted(actions[repository]):
            if parse_version(ref)[0] < parse_version(newest)[0]:
                rows.append((repository, ref, newest))
    return rows


def table(header, rows):
    lines = ["| " + " | ".join(header) + " |", "|" + "---|" * len(header)]
    lines.extend("| " + " | ".join(str(cell).replace("|", "\\|") for cell in row) + " |" for row in rows)
    return "\n".join(lines)


def render_report(sections, errors, run_url):
    lines = [f"# {REPORT_TITLE}", ""]
    if run_url:
        lines += [f"Generated by {run_url}.", ""]
    for title, note, header, rows in sections:
        lines += [f"## {title}", ""]
        if rows:
            if note:
                lines += [note, ""]
            lines += [table(header, rows), ""]
        else:
            lines += ["Nothing to report.", ""]
    if errors:
        lines += ["## Check errors", "", "These checks could not run; their sections above are incomplete.", ""]
        lines += [f"- {error}" for error in errors]
        lines.append("")
    return "\n".join(lines)


def fetch_text(url):
    request = urllib.request.Request(url, headers={"User-Agent": "lapligence-dependency-report"})
    with urllib.request.urlopen(request, timeout=NETWORK_TIMEOUT_SECONDS) as response:
        return response.read().decode("utf-8")


def remote_tags(url):
    result = subprocess.run(["git", "ls-remote", "--tags", url], capture_output=True, text=True,
                            timeout=NETWORK_TIMEOUT_SECONDS * 4, check=True)
    return parse_ls_remote(result.stdout)


def git_output(root, *args):
    return subprocess.run(["git", "-C", str(root), *args], capture_output=True, text=True, check=True).stdout


def gather(root, audit_path):
    errors = []
    sections = []

    vulnerabilities, warnings = [], []
    try:
        vulnerabilities, warnings = audit_findings(json.loads(Path(audit_path).read_text(encoding="utf-8")))
    except (OSError, ValueError, KeyError, TypeError) as error:
        errors.append(f"cargo audit report {audit_path}: {error}")
    sections.append(("RustSec vulnerabilities", "These fail the workflow.",
                     ("Advisory", "Crate", "Locked", "Title", "Patched"), vulnerabilities))
    sections.append(("RustSec warnings", "Unmaintained, unsound or yanked crates in Cargo.lock.",
                     ("Kind", "Advisory", "Crate", "Locked", "Title"), warnings))

    crates = []
    try:
        manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
        lockfile = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))
        direct = direct_dependencies(manifest)
        latest = {}
        for name in sorted(direct):
            try:
                latest[name] = latest_index_version(fetch_text(f"{CRATES_INDEX}/{index_path(name)}"))
            except (OSError, ValueError) as error:
                errors.append(f"crates.io index for {name}: {error}")
        crates = outdated_crates(direct, locked_versions(lockfile), latest)
    except (OSError, tomllib.TOMLDecodeError) as error:
        errors.append(f"Cargo manifests: {error}")
    sections.append(("Outdated direct Cargo dependencies",
                     "`cargo update` stays within the Cargo.toml requirement; a bump changes it.",
                     ("Crate", "Locked", "Latest", "Upgrade", "Declared in"), crates))

    toolchain = []
    try:
        channel = tomllib.loads((root / "rust-toolchain.toml").read_text(encoding="utf-8"))["toolchain"]["channel"]
        newest = latest_version(remote_tags(RUST_REPOSITORY))
        if parse_version(channel) is not None and newest and parse_version(newest) > parse_version(channel):
            toolchain.append((channel, newest))
    except (OSError, KeyError, tomllib.TOMLDecodeError, subprocess.SubprocessError) as error:
        errors.append(f"Rust toolchain: {error}")
    sections.append(("Rust toolchain",
                     "rust-toolchain.toml and every `dtolnay/rust-toolchain@` pin in .github move together.",
                     ("Pinned", "Latest stable"), toolchain))

    submodules = []
    try:
        config = git_output(root, "config", "-f", ".gitmodules", "--get-regexp", r"^submodule\..*\.(path|url)$")
        paths, urls = {}, {}
        for line in config.splitlines():
            key, value = line.split(" ", 1)
            name, field = key[len("submodule."):].rsplit(".", 1)
            (paths if field == "path" else urls)[name] = value
        for name in sorted(paths):
            try:
                gitlink = git_output(root, "ls-tree", "HEAD", paths[name]).split()
                pinned, newest, behind = submodule_status(gitlink[2], remote_tags(urls[name]))
                if behind:
                    note = "re-prepare patches/slang on the new base" if paths[name] == "vendor/slang" else ""
                    submodules.append((paths[name], pinned, newest, note))
            except (IndexError, subprocess.SubprocessError) as error:
                errors.append(f"submodule {paths.get(name, name)}: {error}")
    except subprocess.SubprocessError as error:
        errors.append(f".gitmodules: {error}")
    sections.append(("Vendored submodules", "Pinned gitlink against the newest release tag of its remote.",
                     ("Path", "Pinned", "Latest tag", "Note"), submodules))

    actions = []
    texts = [path.read_text(encoding="utf-8") for path in sorted((root / ".github").rglob("*.yml"))]
    references = action_references(texts)
    newest_actions = {}
    for repository in sorted(references):
        try:
            newest_actions[repository] = latest_version(remote_tags(f"https://github.com/{repository}"))
        except subprocess.SubprocessError as error:
            errors.append(f"action {repository}: {error}")
    actions = outdated_actions(references, newest_actions)
    sections.append(("GitHub Actions", "Pinned major tag behind the newest release.",
                     ("Action", "Pinned", "Latest"), actions))

    return sections, errors, len(vulnerabilities)


def main(argv=None):
    parser = argparse.ArgumentParser(description="Write the dependency audit and upgrade reminder report.")
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--audit-json", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--github-output", type=Path)
    parser.add_argument("--run-url", default="")
    args = parser.parse_args(argv)
    if tomllib is None:
        raise SystemExit("ci_dependency_report.py: reading Cargo.toml needs Python 3.11+ (tomllib) or tomli")

    sections, errors, vulnerabilities = gather(args.root, args.audit_json)
    args.output.write_text(render_report(sections, errors, args.run_url), encoding="utf-8")
    findings = sum(len(rows) for _, _, _, rows in sections)
    counts = {"findings": findings, "vulnerabilities": vulnerabilities, "errors": len(errors)}
    if args.github_output:
        with args.github_output.open("a", encoding="utf-8") as handle:
            handle.writelines(f"{key}={value}\n" for key, value in counts.items())
    print(" ".join(f"{key}={value}" for key, value in counts.items()))
    for error in errors:
        print(f"error: {error}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
