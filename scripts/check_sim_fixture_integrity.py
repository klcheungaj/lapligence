#!/usr/bin/env python3
"""Check the repository's static simulation-fixture references, without Rust.

Recognized harnesses: sim_cli::<name>(suite, stem, ...), fixture/run_fixture
helpers with a literal suite root, and datatype_case!(..., file, label).
This is a bounded harness-integrity check, not a general Rust parser. Dynamically
created temporary input files are not treated as checked-in fixtures. Add a
recognizer when introducing a new checked-in-fixture harness. --tracked also
checks Git's index so an untracked local file cannot conceal a broken patch.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
import posixpath
import re
import subprocess
from typing import Iterable


@dataclass(frozen=True, order=True)
class Reference:
    path: str
    source: str
    line: int


def without_comments(text: str) -> str:
    """Blank Rust comments but retain string literals and line positions."""
    result = list(text)
    i = 0
    while i < len(text):
        raw = re.match(r'r(#+)?"', text[i:])
        if raw:
            end_mark = '"' + (raw.group(1) or '')
            end = text.find(end_mark, i + raw.end())
            i = len(text) if end < 0 else end + len(end_mark)
        elif text[i] == '"':
            i += 1
            while i < len(text):
                if text[i] == "\\":
                    i += 2
                elif text[i] == '"':
                    i += 1
                    break
                else:
                    i += 1
        elif text.startswith("//", i):
            end = text.find("\n", i)
            end = len(text) if end < 0 else end
            result[i:end] = " " * (end - i)
            i = end
        elif text.startswith("/*", i):
            begin, depth = i, 1
            i += 2
            while i < len(text) and depth:
                if text.startswith("/*", i):
                    depth += 1
                    i += 2
                elif text.startswith("*/", i):
                    depth -= 1
                    i += 2
                else:
                    i += 1
            for j in range(begin, i):
                if result[j] != "\n":
                    result[j] = " "
        else:
            i += 1
    return "".join(result)


STRING = r'"([^"\n]+)"'
ATOM = r'(?:"[^"\n]+"|[A-Z][A-Z0-9_]*)'


def source_references(text: str, source: str, *, public_cli_only: bool = False) -> set[Reference]:
    text = without_comments(text)
    constants = dict(re.findall(r'const\s+(\w+)\s*:\s*&str\s*=\s*' + STRING, text))
    refs: set[Reference] = set()

    def atom(value: str) -> str | None:
        return value[1:-1] if value.startswith('"') else constants.get(value)

    def add(path: str, position: int) -> None:
        if "\\" in path or ".." in PurePosixPath(path).parts:
            raise ValueError(f"{source}: non-portable or escaping fixture path: {path}")
        refs.add(Reference(path, source, text.count("\n", 0, position) + 1))

    for m in re.finditer(r'sim_cli::\w+\(\s*(' + ATOM + r')\s*,\s*(' + ATOM + r')', text):
        helper = m[0].split("(", 1)[0].removeprefix("sim_cli::")
        if public_cli_only and (helper == "run_case_after_db_drop"
                                or not helper.startswith(("run_case", "reject_case"))):
            continue
        suite, stem = atom(m[1]), atom(m[2])
        if suite is not None and stem is not None:
            name = stem if Path(stem).suffix in (".v", ".sv") else f"{stem}.sv"
            add(f"tests/fixtures/sim/{suite}/{name}", m.start())

    for m in re.finditer(
        r'sim_cli::run_case_with_source_prefix\(\s*'
        + r'('
        + ATOM
        + r')'
        + r'\s*,\s*'
        + r'('
        + ATOM
        + r')'
        + r'\s*,\s*&\s*\[([^\]]*)\]',
        text,
    ):
        suite = atom(m[1])
        for stem_match in re.finditer(ATOM, m[3]):
            stem = atom(stem_match[0])
            if suite is not None and stem is not None:
                name = stem if Path(stem).suffix in (".v", ".sv") else f"{stem}.sv"
                add(f"tests/fixtures/sim/{suite}/{name}", m.start())

    for m in re.finditer(
        r'sim_cli::(?:run|reject)_case_with_inputs\(\s*'
        + r'(' + ATOM + r')\s*,\s*(?:' + ATOM + r')\s*,\s*&\s*\[([^\]]*)\]',
        text,
    ):
        suite = atom(m[1])
        for input_match in re.finditer(ATOM, m[2]):
            name = atom(input_match[0])
            if suite is not None and name is not None:
                add(f"tests/fixtures/sim/{suite}/{name}", m.start())

    if public_cli_only:
        return refs

    for m in re.finditer(r'include_str!\(\s*' + STRING + r'\s*\)', text):
        path = posixpath.normpath(str(PurePosixPath(source).parent / m[1]))
        if path.startswith("tests/fixtures/"):
            add(path, m.start())


    joins = list(re.finditer(r'\.join\(\s*' + STRING + r'\s*\)', text))
    roots = {m[1] for m in joins if m[1].startswith("tests/fixtures/sim/")
             and not Path(m[1]).suffix}
    for m in joins:
        if m[1].startswith("tests/fixtures/") and Path(m[1]).suffix:
            add(m[1], m.start())
    if len(roots) == 1:
        root = next(iter(roots))
        for m in re.finditer(r'fixture!\(\s*(\w+)\s*,\s*"(v|sv)"\s*\)', text):
            add(f"{root}/{m[1]}.{m[2]}", m.start())
        for m in re.finditer(r'datatype_case!\(\s*\w+\s*,\s*' + STRING, text):
            add(f"{root}/{m[1]}", m.start())
        for m in re.finditer(r'(?:run_fixture\w*|reject_fixture\w*|fixture_rejection|fixture_path|run_rejection_fixture|with_compiled_fixture|run_assignment_pattern_rejection_fixture|fixture)\(\s*' + STRING, text):
            if Path(m[1]).suffix in (".sv", ".v", ".svh", ".vh"):
                add(f"{root}/{m[1]}", m.start())
    return refs


def collect(root: Path) -> set[Reference]:
    references: set[Reference] = set()
    for source in sorted((root / "tests").rglob("*.rs")):
        references.update(source_references(source.read_text(encoding="utf-8"),
                                             source.relative_to(root).as_posix()))
    return references


def missing(root: Path, refs: Iterable[Reference], tracked: set[str] | None = None) -> list[str]:
    errors = []
    for ref in sorted(refs):
        if not (root / ref.path).is_file():
            errors.append(f"{ref.source}:{ref.line}: missing {ref.path}")
        elif tracked is not None and ref.path not in tracked:
            errors.append(f"{ref.source}:{ref.line}: untracked {ref.path}")
    return errors


def test_bodies(text: str) -> list[str]:
    text = without_comments(text)
    masked = re.sub(r'r(#+)?".*?"\1|"(?:\\.|[^"\\])*"',
                    lambda m: " " * len(m[0]), text, flags=re.DOTALL)
    bodies = []
    for match in re.finditer(r'#\[test\]\s*fn\s+\w+\s*\([^)]*\)\s*\{', masked):
        start, depth, end = match.end(), 1, match.end()
        while end < len(masked) and depth:
            depth += (masked[end] == "{") - (masked[end] == "}")
            end += 1
        if depth == 0:
            bodies.append(text[start:end - 1])
    return bodies


def feature_completion_errors(root: Path, tracked: set[str] | None = None) -> list[str]:
    fixture_root = root / "tests/fixtures/sim/feature_completion"
    directories = sorted(path for path in fixture_root.iterdir() if path.is_dir()
                         and not re.fullmatch(r"g1_\d+", path.name)) if fixture_root.is_dir() else []
    suite = "tests/sim_feature_completion.rs"
    if not directories and not (root / suite).is_file():
        return []
    errors = []
    if not (root / suite).is_file():
        return [f"missing declared feature suite: {suite}"]
    facade = without_comments((root / suite).read_text(encoding="utf-8"))
    if re.search(r'#\[\s*(?:ignore|cfg|cfg_attr)\b', facade):
        errors.append(f"{suite}: feature modules must not be disabled or conditional")
    declarations = set(re.findall(
        r'#\[path\s*=\s*"sim_feature_completion/(\w+)\.rs"\]\s*mod\s+\1\s*;', facade))
    if not declarations:
        errors.append(f"{suite}: zero declared feature modules")
    if tracked is not None and suite not in tracked:
        errors.append(f"untracked {suite}")
    for directory in directories:
        task = directory.name
        if not re.fullmatch(r"(?:fnd|rtl|sim|adv)_\d{3}[a-z]?", task):
            errors.append(f"{directory.relative_to(root)}: use an ASCII task slug (e.g. rtl_001)")
        source = f"tests/sim_feature_completion/{task}.rs"
        if task not in declarations or not (root / source).is_file():
            errors.append(f"{task}: missing explicit module declaration or file: {source}")
            continue
        if tracked is not None and source not in tracked:
            errors.append(f"untracked {source}")
        text = without_comments((root / source).read_text(encoding="utf-8"))
        if re.search(r'#\[\s*(?:ignore|cfg|cfg_attr)\b', text):
            errors.append(f"{source}: feature tests must not be disabled or conditional")
        bodies = test_bodies(text)
        if not bodies:
            errors.append(f"{source}: zero declared feature tests")
        constants = "\n".join(re.findall(r'const\s+\w+\s*:\s*&str\s*=\s*"[^"\n]*"\s*;', text))
        referenced = {ref.path for body in bodies
                      for ref in source_references(constants + "\n" + body, source,
                                                   public_cli_only=True)}
        fixtures = sorted(path for path in directory.rglob("*")
                          if path.suffix in (".v", ".sv"))
        if not fixtures:
            errors.append(f"{task}: zero HDL fixtures")
        for fixture in fixtures:
            path = fixture.relative_to(root).as_posix()
            if not fixture.name.isascii():
                errors.append(f"{path}: fixture name must be ASCII")
            if path not in referenced:
                errors.append(f"{path}: not referenced by a declared feature test through public sim_cli")
    for task in sorted(declarations - {directory.name for directory in directories}):
        errors.append(f"{task}: declared feature module has no fixture directory")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--tracked", action="store_true", help="require each input in Git's index")
    args = parser.parse_args()
    root = args.root.resolve()
    if not (root / "tests").is_dir():
        parser.error(f"no tests directory in {root}")
    try:
        refs = collect(root)
        if not refs:
            raise ValueError("no supported fixture references were discovered")
        tracked = None
        if args.tracked:
            output = subprocess.run(["git", "-C", str(root), "ls-files", "-z"],
                                    check=True, capture_output=True)
            tracked = set(output.stdout.decode("utf-8").split("\0"))
        errors = missing(root, refs, tracked) + feature_completion_errors(root, tracked)
    except (OSError, ValueError, subprocess.CalledProcessError) as exc:
        parser.error(str(exc))
    for error in errors:
        print(error)
    print(f"fixture integrity: {len(refs)} static references, "
          f"{len({ref.path for ref in refs})} distinct paths, {len(errors)} errors")
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
