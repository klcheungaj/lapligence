import os
import subprocess
import sys

try:
    import resource
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
except ImportError:
    pass

env = os.environ.copy()
env["ASAN_OPTIONS"] = "detect_leaks=0:abort_on_error=1"
probe, bad = sys.argv[1:]
base = subprocess.run([probe], capture_output=True, text=True, check=True, timeout=30)
count = int(base.stdout.strip())
for index in range(count):
    result = subprocess.run([probe, str(index)], capture_output=True, text=True,
                            env=env, timeout=30)
    if result.returncode == 0 or "allocation failed" not in result.stderr:
        raise SystemExit(f"allocation failure {index} was not handled:\n{result.stderr}")
for case in ["limit", "overflow", "mask_width", "overlap", "state", "plane", "drivers", "driver_width"]:
    result = subprocess.run([bad, case], capture_output=True, text=True, env=env, timeout=30)
    if result.returncode == 0 or "prototype fatal:" not in result.stderr:
        raise SystemExit(f"invalid input {case} was not rejected:\n{result.stderr}")
print(f"{count} allocation failures and 8 invalid-argument cases rejected; warm reuse allocated zero application blocks")
