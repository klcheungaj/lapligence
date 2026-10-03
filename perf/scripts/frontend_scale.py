import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import threading
import time

parser = argparse.ArgumentParser()
parser.add_argument('binary')
parser.add_argument('output', type=Path)
parser.add_argument('--time', type=Path, default=Path('/usr/bin/time'))
parser.add_argument('--sample-ms', type=int, default=5)
parser.add_argument('--counts', nargs='+', type=int, default=[5000, 10000, 20000, 40000])
parser.add_argument('--runs', type=int, default=3)
args = parser.parse_args()
if not sys.platform.startswith('linux'):
    parser.error('stage RSS sampling requires Linux /proc')
if args.runs < 3 or args.sample_ms < 1 or any(count < 1 for count in args.counts):
    parser.error('use at least three runs, positive counts and a positive sample interval')
if not args.time.is_file():
    parser.error('GNU time was not found; provide its path with --time')
root = args.output.resolve()
root.mkdir(parents=True, exist_ok=True)
for count in args.counts:
    for run in range(args.runs):
        out = root / f'{count}-{run}'
        out.mkdir(exist_ok=True)
        cmd = [str(args.time.resolve()), '-v', '-o', str(out / 'time.txt'), args.binary,
               '--gen-only', '--max-export-mib', '4096', '--top', 'many_processes_registers_config',
               '--define', f'LLG_CORPUS_N={count}', '--define', 'LLG_CORPUS_EDGES=2',
               '--out-dir', str(out), 'perf/corpus/many_processes.sv']
        env = dict(os.environ, LLG_PROFILE_STAGES='1')
        stages = {}
        active = []
        pid = [None]
        stopped = threading.Event()
        def sample():
            while not stopped.wait(args.sample_ms / 1000):
                if pid[0] is None:
                    continue
                try:
                    rss = int(Path(f'/proc/{pid[0]}/statm').read_text().split()[1]) * os.sysconf('SC_PAGE_SIZE')
                except (OSError, IndexError, ValueError):
                    continue
                for stage in tuple(active):
                    record = stages[stage]
                    record['peak_rss'] = max(record['peak_rss'], rss)
        thread = threading.Thread(target=sample, daemon=True)
        thread.start()
        started = time.monotonic()
        with (out / 'stdout.txt').open('w') as stdout, (out / 'stderr.txt').open('w') as stderr:
            proc = subprocess.Popen(cmd, stdout=stdout, stderr=subprocess.PIPE, text=True, env=env)
            for line in proc.stderr:
                stderr.write(line)
                match = re.match(r'llg-profile begin (\S+)(?: pid=(\d+))?', line)
                if match:
                    stage, process = match.groups()
                    if process is not None:
                        pid[0] = int(process)
                    stages[stage] = dict(peak_rss=0)
                    active.append(stage)
                match = re.match(r'llg-profile end (\S+) seconds=(\S+)', line)
                if match:
                    stage, seconds = match.groups()
                    stages[stage]['seconds'] = float(seconds)
                    active.remove(stage)
            status = proc.wait()
        stopped.set()
        thread.join()
        models = list(out.glob('sim/*/model.c'))
        if status == 0 and len(models) != 1:
            raise RuntimeError('successful generation must produce exactly one model.c')
        digest = hashlib.sha256(models[0].read_bytes()).hexdigest() if models else None
        result = dict(count=count, run=run, status=status, seconds=time.monotonic() - started, stages=stages, sha256=digest)
        (out / 'metrics.json').write_text(json.dumps(result, indent=2) + '\n')
        print(json.dumps(result), flush=True)
        if status:
            raise SystemExit(status)
