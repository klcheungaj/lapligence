#!/usr/bin/env python3
import argparse
import hashlib
import json
import sys
import tarfile
import tomllib
import zipfile
from pathlib import Path

PLATFORMS = (
    "linux-x86_64",
    "linux-arm64",
    "windows-x86_64",
    "windows-arm64",
    "macos-arm64",
)
LSP_ONLY_PLATFORMS = frozenset({"windows-arm64"})


def package_version(manifest):
    with Path(manifest).open("rb") as handle:
        return tomllib.load(handle)["package"]["version"]


def tag_problem(tag, version):
    if tag == f"v{version}":
        return None
    return f"tag {tag} does not name the package version {version} (expected v{version})"


def archive_name(version, platform):
    extension = "zip" if platform.startswith("windows-") else "tar.gz"
    return f"lapligence-{version}-{platform}.{extension}"


def artifact_names():
    return [f"lapligence-{platform}" for platform in PLATFORMS]


def release_files(version):
    names = []
    for platform in PLATFORMS:
        archive = archive_name(version, platform)
        names += [archive, f"{archive}.sha256"]
    return names


def expected_members(platform):
    suffix = ".exe" if platform.startswith("windows-") else ""
    members = {f"llg_ls{suffix}", "LICENSE", "readme.md"}
    if platform not in LSP_ONLY_PLATFORMS:
        members.add(f"llg{suffix}")
    return members


def archive_members(path):
    if path.name.endswith(".zip"):
        with zipfile.ZipFile(path) as archive:
            return archive.namelist()
    with tarfile.open(path) as archive:
        return archive.getnames()


def digest(path):
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            hasher.update(block)
    return hasher.hexdigest()


def verify(version, directory):
    directory = Path(directory)
    present = sorted(path.name for path in directory.iterdir() if path.is_file())
    expected = sorted(release_files(version))
    if present != expected:
        return [f"{directory} holds {present}, expected {expected}"]
    problems = []
    for platform in PLATFORMS:
        archive = directory / archive_name(version, platform)
        recorded = (directory / f"{archive.name}.sha256").read_text(encoding="ascii").split()
        actual = [digest(archive), archive.name]
        if recorded != actual:
            problems.append(f"{archive.name}.sha256 records {recorded}, expected {actual}")
        try:
            members = sorted(archive_members(archive))
        except (OSError, tarfile.TarError, zipfile.BadZipFile) as error:
            problems.append(f"{archive.name}: unreadable archive: {error}")
            continue
        wanted = sorted(expected_members(platform))
        if members != wanted:
            problems.append(f"{archive.name} holds {members}, expected {wanted}")
    return problems


def asset_problems(directory, assets):
    attached = {asset["name"]: asset for asset in assets}
    problems = []
    for path in sorted(Path(directory).iterdir()):
        if not path.is_file():
            continue
        asset = attached.get(path.name)
        if asset is None:
            problems.append(f"{path.name}: not attached to the release")
        elif asset.get("state") != "uploaded":
            problems.append(f"{path.name}: release asset state is {asset.get('state')}")
        elif asset.get("size") != path.stat().st_size:
            problems.append(
                f"{path.name}: release asset holds {asset.get('size')} bytes, "
                f"expected {path.stat().st_size}"
            )
    return problems


def report(problems):
    for problem in problems:
        print(f"error: {problem}", file=sys.stderr)
    return 1 if problems else 0


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="Name, list and check the release packages CI builds and the "
        "release workflow attaches."
    )
    parser.add_argument("--manifest", default="Cargo.toml")
    commands = parser.add_subparsers(dest="command", required=True)
    version_command = commands.add_parser(
        "version", help="print the package version; with --tag, fail unless the tag names it"
    )
    version_command.add_argument("--tag")
    archive_command = commands.add_parser("archive", help="print one platform's archive name")
    archive_command.add_argument("platform", choices=PLATFORMS)
    commands.add_parser("artifacts", help="print the CI artifact name of each package")
    verify_command = commands.add_parser(
        "verify", help="check a directory holds exactly the packages and their checksums"
    )
    verify_command.add_argument("directory")
    assets_command = commands.add_parser(
        "assets",
        help="check every file of a directory is an uploaded asset of the same size "
        "(release assets as JSON lines on stdin)",
    )
    assets_command.add_argument("directory")
    args = parser.parse_args(argv)

    if args.command == "artifacts":
        print("\n".join(artifact_names()))
        return 0
    if args.command == "assets":
        assets = [json.loads(line) for line in sys.stdin if line.strip()]
        return report(asset_problems(args.directory, assets))
    version = package_version(args.manifest)
    if args.command == "version":
        problem = tag_problem(args.tag, version) if args.tag is not None else None
        if problem is not None:
            return report([problem])
        print(version)
        return 0
    if args.command == "archive":
        print(archive_name(version, args.platform))
        return 0
    return report(verify(version, args.directory))


if __name__ == "__main__":
    sys.exit(main())
