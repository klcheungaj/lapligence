#!/usr/bin/env python3
import argparse
import concurrent.futures
import json
import os
import subprocess
import sys
import threading

NONBLOCKING = ("nba", "mnba")
PREFIX = "feature_completion/" + os.path.basename(os.path.dirname(os.path.abspath(__file__))) + "/"
HERE = os.path.dirname(os.path.abspath(__file__))
ROWS_FILE = os.path.join(HERE, "rows.txt")
ALLOW = os.path.normpath(os.path.join(HERE, "..", "..", "lint", "allow_mixed_assignments.toml"))

TYPES = {
    "str": "int a; string s; int arr[`N];",
    "pk": "int a; logic [39:0] b; int arr[`N]; bit [3:0] c; byte few[2];",
    "mix": "int a; string s; byte m[2][3]; real d; int few[2]; int arr[`N];",
    "nest": "int a; inner_t in; inner_t ie[2]; int arr[`N];",
    "rl": "int a; string s; real arr[`N];",
    "st": "int a; string arr[`N];",
    "obj": "int a; obj_t arr[`N];",
}

SRC = {
    "mod": "mod",
    "loc": "loc",
    "stat": "stat",
    "call": "mk()",
    "ident": "id(loc)",
    "q": "q[0]",
    "dyn": "dyn[0]",
    "as": "as[1]",
    "fa": "fa[1]",
    "nas": 'nas["x"][1]',
    "naq": "qq[0][0]",
    "cls": "c.r",
    "mem": "o.e[1]",
    "emem": "oq[0].e[1]",
    "pop": "q.pop_back()",
    "cond": "sel ? q[0] : loc",
    "cond2": "sel ? mk() : as[1]",
    "pat": "'{default:0}",
    "var": "q[sel]",
}

TGT = {
    "mod": "mod = {S};",
    "loc": "loc = {S};",
    "stat": "stat = {S};",
    "q": "q[0] = {S};",
    "push": "q.push_back({S});",
    "dyn": "dyn[1] = {S};",
    "as": "as[2] = {S};",
    "fa": "fa[2] = {S};",
    "nas": 'nas["x"][2] = {S};',
    "naq": "qq[0][1] = {S};",
    "cls": "c.r = {S};",
    "mem": "o.e[0] = {S};",
    "emem": "oq[0].e[0] = {S};",
    "out": "outp(loc, {S});",
    "ref": "rf(q[0], {S});",
    "init": "begin automatic rec_t z = {S}; loc = z; end",
    "cmp": "cnt += (loc == {S});",
    "arg": "mod = id({S});",
    "arg2": "q[0] = id({S});",
    "var": "q[sel] = {S};",
    "nba": "fa[2] <= {S};",
    "mnba": "mod <= {S};",
}

READ = {
    "mod": "mod",
    "loc": "loc",
    "stat": "stat",
    "q": "q[0]",
    "push": "q[$]",
    "dyn": "dyn[1]",
    "as": "as[2]",
    "fa": "fa[2]",
    "nas": 'nas["x"][2]',
    "naq": "qq[0][1]",
    "cls": "c.r",
    "mem": "o.e[0]",
    "emem": "oq[0].e[0]",
    "out": "loc",
    "ref": "q[0]",
    "init": "loc",
    "cmp": "loc",
    "arg": "mod",
    "arg2": "q[0]",
    "var": "q[sel]",
    "nba": "fa[2]",
    "mnba": "mod",
}

HOLDERS = [
    ("mod", 1),
    ("loc", 2),
    ("stat", 3),
    ("q[0]", 4),
    ("dyn[0]", 5),
    ("as[1]", 6),
    ("fa[1]", 7),
    ('nas["x"][1]', 8),
    ("qq[0][0]", 9),
    ("o.e[1]", 11),
]


def fill(kind, x, a):
    if kind == "str":
        return f'{x}.a={a}; {x}.s="s"; {x}.arr[2]={a + 1};'
    if kind == "pk":
        return (
            f"{x}.a={a}; {x}.b=40'h12_0000_0000; {x}.c=5; {x}.arr[2]={a + 1}; {x}.few[1]=8;"
        )
    if kind == "mix":
        return (
            f'{x}.a={a}; {x}.s="s"; {x}.m[1][2]=7; {x}.d=1.5; {x}.few[1]=2; {x}.arr[2]={a + 1};'
        )
    if kind == "nest":
        return f"{x}.a={a}; {x}.in.u=3; {x}.ie[1].u=5; {x}.arr[2]={a + 1};"
    if kind == "rl":
        return f'{x}.a={a}; {x}.s="s"; {x}.arr[2]={a}.5;'
    if kind == "st":
        return f'{x}.a={a}; {x}.arr[2]=$sformatf("v%0d",{a});'
    return f"{x}.a={a}; {x}.arr[2]=mo({a});"


def show(kind, x):
    return {
        "str": f'$display("%0d %s %0d", {x}.a, {x}.s, {x}.arr[2]);',
        "pk": f'$display("%0d %h %0d %0d %0d", {x}.a, {x}.b, {x}.c, {x}.arr[2], {x}.few[1]);',
        "mix": f'$display("%0d %s %0d %0.1f %0d %0d", {x}.a, {x}.s, {x}.m[1][2], {x}.d, {x}.few[1], {x}.arr[2]);',
        "nest": f'$display("%0d %0d %0d %0d", {x}.a, {x}.in.u, {x}.ie[1].u, {x}.arr[2]);',
        "rl": f'$display("%0d %s %0.1f", {x}.a, {x}.s, {x}.arr[2]);',
        "st": f'$display("%0d %s", {x}.a, {x}.arr[2]);',
        "obj": f'$display("%0d %0d", {x}.a, {x}.arr[2]==null?-1:{x}.arr[2].v);',
    }[kind]


def header(kind):
    return f"""`ifndef N
`define N 16
`endif
class obj_t; int v; function new(int x); v = x; endfunction endclass
typedef struct {{ int u; int v[2]; }} inner_t;
typedef struct {{ {TYPES[kind]} }} rec_t;
typedef struct {{ rec_t e[2]; int t; }} o_t;
class C; rec_t r; endclass
module tb;
  rec_t mod, q[$], dyn[], as[int], fa[3], nas[string][int], qq[$][$];
  o_t o, oq[$];
  int sel = 0, cnt = 0;
  function automatic obj_t mo(int x); mo = new(x); endfunction
  function automatic rec_t mk(); rec_t r; {fill(kind, "r", 13)} return r; endfunction
  function automatic rec_t id(rec_t x); return x; endfunction
  task automatic outp(output rec_t x, input rec_t y); x = y; endtask
  task automatic rf(ref rec_t x, input rec_t y); x = y; endtask
"""


def row(kind, src, tgt, number):
    holders = "\n".join("    " + fill(kind, x, a) for x, a in HOLDERS)
    write = TGT[tgt].replace("{S}", SRC[src])
    return f"""  initial begin : row{number}
    static rec_t stat;
    automatic rec_t loc;
    automatic C c = new;
    automatic rec_t e;
    automatic rec_t row[$];
    #({number * 10});
    sel = 0; cnt = 0;
    q.delete(); dyn = new[2]; as.delete(); nas.delete(); qq.delete(); oq.delete();
    q.push_back(e); row.push_back(e); row.push_back(e); qq.push_back(row);
{holders}
    c.r = mk();
    oq.push_back(o);
    $display("row {number} {kind} {src} {tgt}");
    {write}
    #1;
    {show(kind, READ[tgt])}
    $display("%0d %0d", cnt, q.size());
  end
"""


def footer(count):
    return f"""  initial begin
    #({(count + 1) * 10});
    $finish(0);
  end
endmodule
"""


def source(kind, rows):
    body = "".join(row(kind, s, g, n) for n, (s, g) in enumerate(rows, 1))
    return header(kind) + body + footer(len(rows))


def read_rows():
    table = {}
    with open(ROWS_FILE) as handle:
        for line in handle:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            kind, src, targets = line.split(" ", 2)
            table.setdefault(kind, {})[src] = targets.split(",")
    return table


def covering(table):
    plan = {}
    targets = list(TGT)
    sources = list(SRC)
    for position, kind in enumerate(TYPES):
        chosen = []
        for index, tgt in enumerate(targets):
            start = (index + 5 * position) % len(sources)
            for step in range(len(sources)):
                src = sources[(start + step) % len(sources)]
                if tgt in table.get(kind, {}).get(src, []):
                    chosen.append((src, tgt))
                    break
        used = {src for src, _ in chosen}
        for src in sources:
            if src not in used and table.get(kind, {}).get(src):
                chosen.append((src, table[kind][src][0]))
        plan[kind] = chosen
    return plan


def split_rows(rows):
    plain = [(s, g) for s, g in rows if g not in NONBLOCKING]
    queued = [(s, g) for s, g in rows if g in NONBLOCKING]
    return plain, queued


def named_sources(prefix, kind, rows):
    plain, queued = split_rows(rows)
    files = {}
    if plain:
        files[f"{prefix}.sv"] = source(kind, plain)
    if queued:
        files[f"{prefix}_nba.sv"] = source(kind, queued)
    return files


def covering_files(table):
    files = {}
    for kind, rows in covering(table).items():
        files.update(named_sources(f"shapes_{kind}", kind, rows))
    return files


def full_files(table):
    files = {}
    for kind, by_source in table.items():
        for src, targets in by_source.items():
            rows = [(src, tgt) for tgt in targets]
            files.update(named_sources(f"{kind}_{src}", kind, rows))
    return files


def lint_lines(files):
    lines = []
    for name, text in files.items():
        if not name.endswith("_nba.sv"):
            continue
        for number, line in enumerate(text.split("\n"), 1):
            if line.startswith("  initial begin : row"):
                lines.append(f"{PREFIX}{name}\tmixed-assignments\t{number}:3")
    return lines


def lint_manifest():
    prefix = PREFIX
    path = os.path.join(HERE, "..", "..", "lint", "expected_errors.tsv")
    with open(path) as handle:
        return sorted(line.rstrip("\n") for line in handle if line.startswith(prefix))


def write_tree(directory, files):
    os.makedirs(directory, exist_ok=True)
    for name, text in files.items():
        with open(os.path.join(directory, name), "w") as handle:
            handle.write(text)


def check_tree(directory, files):
    stale = []
    for name, text in files.items():
        path = os.path.join(directory, name)
        if not os.path.exists(path) or open(path).read() != text:
            stale.append(name)
    present = {name for name in os.listdir(directory) if name.startswith("shapes_") and name.endswith(".sv")}
    stale.extend(sorted(present - set(files)))
    return stale


def survey_one(compiler, work, cache, kind, src, tgt):
    results = []
    for cells in (16, 3):
        directory = os.path.join(work, f"{kind}_{src}_{tgt}_{cells}")
        os.makedirs(directory, exist_ok=True)
        with open(os.path.join(directory, "t.sv"), "w") as handle:
            handle.write(source(kind, [(src, tgt)]))
        environment = dict(os.environ, LLG_RUNTIME_CACHE_DIR=cache)
        try:
            run = subprocess.run(
                [compiler, "--config", ALLOW, "-D", f"N={cells}", "--top", "tb", "t.sv"],
                cwd=directory,
                env=environment,
                capture_output=True,
                text=True,
                timeout=300,
            )
            results.append((run.returncode, run.stdout))
        except subprocess.TimeoutExpired:
            results.append((-1, ""))
        subprocess.run(["rm", "-rf", directory])
    return (kind, src, tgt), results


def survey(compiler, work, jobs):
    if not os.path.isfile(ALLOW):
        raise SystemExit(f"missing {ALLOW}")
    os.makedirs(work, exist_ok=True)
    cache = os.path.join(work, "runtime-cache")
    log = os.path.join(work, "survey.jsonl")
    done = {}
    if os.path.exists(log):
        with open(log) as handle:
            for line in handle:
                record = json.loads(line)
                done[tuple(record["row"])] = record["results"]
    todo = [(k, s, g) for k in TYPES for s in SRC for g in TGT if (k, s, g) not in done]
    lock = threading.Lock()
    with open(log, "a") as handle, concurrent.futures.ThreadPoolExecutor(jobs) as pool:
        futures = [pool.submit(survey_one, compiler, work, cache, *item) for item in todo]
        for future in futures:
            row, results = future.result()
            done[row] = results
            with lock:
                handle.write(json.dumps({"row": row, "results": results}) + "\n")
                handle.flush()
    both = {}
    wide_only = {}
    broken = []
    for (kind, src, tgt), ((rc16, out16), (rc3, out3)) in done.items():
        if rc16 == 0 and rc3 == 0 and out16 == out3:
            both.setdefault(kind, {}).setdefault(src, []).append(tgt)
        elif rc16 == 0 and rc3 != 0:
            wide_only.setdefault(kind, {}).setdefault(src, []).append(tgt)
        elif rc16 != 0 and rc3 != 0:
            continue
        else:
            broken.append((kind, src, tgt, rc16, rc3))
    for table in (both, wide_only):
        for by_source in table.values():
            for src, targets in by_source.items():
                targets.sort(key=list(TGT).index)
    return both, wide_only, broken


def format_rows(both, wide_only):
    lines = [
        "# type source targets: rows that build and print the same at 16 and at 3 cells",
        "# regenerate with: gen_record_shapes.py survey <llg> <work dir>",
    ]
    for kind in TYPES:
        for src in SRC:
            targets = both.get(kind, {}).get(src)
            if targets:
                lines.append(f"{kind} {src} {','.join(targets)}")
    lines.append("# rows that build only at 16 cells (no per-cell form to compare with)")
    for kind in TYPES:
        for src in SRC:
            targets = wide_only.get(kind, {}).get(src)
            if targets:
                lines.append(f"# {kind} {src} {','.join(targets)}")
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("write")
    commands.add_parser("check")
    full = commands.add_parser("full")
    full.add_argument("directory")
    probe = commands.add_parser("survey")
    probe.add_argument("compiler")
    probe.add_argument("work")
    probe.add_argument("--jobs", type=int, default=4)
    arguments = parser.parse_args()
    if arguments.command == "survey":
        both, wide_only, broken = survey(arguments.compiler, arguments.work, arguments.jobs)
        with open(ROWS_FILE, "w") as handle:
            handle.write(format_rows(both, wide_only))
        for item in broken:
            print("BROKEN", *item)
        print("rows", sum(len(t) for k in both.values() for t in k.values()), "broken", len(broken))
        return 1 if broken else 0
    table = read_rows()
    if arguments.command == "write":
        files = covering_files(table)
        write_tree(HERE, files)
        for line in sorted(lint_lines(files)):
            print(line)
        return 0
    if arguments.command == "full":
        write_tree(arguments.directory, full_files(table))
        return 0
    files = covering_files(table)
    stale = check_tree(HERE, files)
    if sorted(lint_lines(files)) != lint_manifest():
        stale.append("lint/expected_errors.tsv")
    for name in stale:
        print("stale", name)
    return 1 if stale else 0


if __name__ == "__main__":
    sys.exit(main())
