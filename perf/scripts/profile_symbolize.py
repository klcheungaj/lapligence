#!/usr/bin/env python3

import argparse
import collections
import functools
import struct
import subprocess
from pathlib import Path

MAGIC = b"LLGPROF\0"
HEADER = struct.Struct("=8sIIII")
RECORD_PREFIX = struct.Struct("=HHI")


def read_samples(path):
    data = Path(path).read_bytes()
    if len(data) < HEADER.size:
        raise ValueError("profile is shorter than its header")
    magic, version, address_size, max_depth, _ = HEADER.unpack_from(data)
    if magic != MAGIC or version != 1:
        raise ValueError("unsupported profile format")
    if address_size not in (4, 8) or max_depth == 0:
        raise ValueError("invalid profile dimensions")
    address_format = "I" if address_size == 4 else "Q"
    record = struct.Struct(f"=HHI{max_depth}{address_format}")
    payload = data[HEADER.size :]
    if len(payload) % record.size:
        raise ValueError("profile ends with a partial sample")
    samples = []
    for offset in range(0, len(payload), record.size):
        values = record.unpack_from(payload, offset)
        depth = min(values[0], max_depth)
        samples.append(list(values[3 : 3 + depth]))
    return samples


def read_maps(path):
    mappings = []
    for line in Path(path).read_text(encoding="utf-8", errors="replace").splitlines():
        fields = line.split(maxsplit=5)
        if len(fields) < 5:
            continue
        start_text, end_text = fields[0].split("-", 1)
        pathname = fields[5] if len(fields) == 6 else ""
        mappings.append(
            (int(start_text, 16), int(end_text, 16), int(fields[2], 16), pathname)
        )
    return mappings


@functools.lru_cache(maxsize=None)
def elf_type(path):
    try:
        with open(path, "rb") as stream:
            header = stream.read(20)
    except OSError:
        return None
    if len(header) < 18 or header[:4] != b"\x7fELF":
        return None
    endian = "<" if header[5] == 1 else ">"
    return struct.unpack_from(endian + "H", header, 16)[0]


def locate(address, mappings):
    for start, end, offset, pathname in mappings:
        if start <= address < end and pathname.startswith("/"):
            object_address = address if elf_type(pathname) == 2 else address - start + offset
            return pathname, object_address
    return None, address


def symbolize(samples, mappings, addr2line):
    keys = {locate(address, mappings) for sample in samples for address in sample}
    cache = {
        key: f"0x{key[1]:x}"
        for key in keys
        if key[0] is None
    }
    by_object = collections.defaultdict(list)
    for pathname, object_address in keys:
        if pathname is not None:
            by_object[pathname].append(object_address)

    for pathname, addresses in by_object.items():
        addresses.sort()
        for start in range(0, len(addresses), 1024):
            batch = addresses[start : start + 1024]
            completed = subprocess.run(
                [
                    addr2line,
                    "-f",
                    "-C",
                    "-e",
                    pathname,
                    *(f"0x{address:x}" for address in batch),
                ],
                check=False,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
            )
            lines = completed.stdout.splitlines()
            for index, address in enumerate(batch):
                line_index = index * 2
                function = lines[line_index] if line_index < len(lines) else "??"
                if function == "??":
                    function = Path(pathname).name
                cache[(pathname, address)] = function

    result = []
    for sample in samples:
        stack = []
        for address in sample:
            key = locate(address, mappings)
            stack.append(cache[key])
        result.append(stack)
    return result


def write_profiles(stacks, flat_path, folded_path):
    flat = collections.Counter()
    folded = collections.Counter()
    for stack in stacks:
        cleaned = [name for name in stack if "sample_signal" not in name]
        flat.update(cleaned)
        if cleaned:
            folded[";".join(reversed(cleaned))] += 1
    with open(flat_path, "w", encoding="utf-8") as stream:
        stream.write("samples\tfunction\n")
        for function, count in sorted(flat.items(), key=lambda item: (-item[1], item[0])):
            stream.write(f"{count}\t{function}\n")
    with open(folded_path, "w", encoding="utf-8") as stream:
        for stack, count in sorted(folded.items(), key=lambda item: (-item[1], item[0])):
            stream.write(f"{stack} {count}\n")


def main():
    parser = argparse.ArgumentParser(description="Symbolize llg SIGPROF samples")
    parser.add_argument("profile")
    parser.add_argument("--maps", help="maps file (default: PROFILE.maps)")
    parser.add_argument("--flat", help="flat output (default: PROFILE.flat.tsv)")
    parser.add_argument("--folded", help="folded output (default: PROFILE.folded)")
    parser.add_argument("--addr2line", default="addr2line")
    args = parser.parse_args()
    maps_path = args.maps or args.profile + ".maps"
    flat_path = args.flat or args.profile + ".flat.tsv"
    folded_path = args.folded or args.profile + ".folded"
    samples = read_samples(args.profile)
    stacks = symbolize(samples, read_maps(maps_path), args.addr2line)
    write_profiles(stacks, flat_path, folded_path)
    print(f"samples={len(samples)} flat={flat_path} folded={folded_path}")


if __name__ == "__main__":
    main()
