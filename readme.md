# Lapligence (`llg`)

Lapligence shares a Slang frontend and owned Rust analysis between two programs:

- **`llg`** compiles/elaborates HDL, runs 24 configurable lint rules, and emits
  optimized C11 that CMake builds and executes. It supports source-only output,
  readable/JSON lint reports and VCD/FST waveform tasks.
- **`llg_ls`** is a stdio language server with compiler/lint diagnostics, semantic
  highlighting, hover, completion, definitions/references, symbols, rename and a
  read-only hierarchy explorer. It tracks unsaved buffers and configuration for
  independent workspace roots.

## Simplified simulator workflow

```text
Verilog/SystemVerilog
        |
        v
 Parse and elaborate (using Slang)
        |
        v
       IR
        |
        +----> lint and editor analysis ----> llg_ls
        |
        +----> optimize ----> emit C11 ----> build ----> run ----> llg
```
The language implementation is incomplete. See
[simulator feature status](docs/sim_features.md) for supported forms/limits and
[lint rules](src/core/lint/readme.md) for linting. The first native frontend build
can take several minutes and use several gigabytes.

## Platform support

The release workflow is configured to build `llg` and `llg_ls` for these
targets:

| Platform | Target | `llg_ls`, lint, and C generation | Generated simulator |
| --- | --- | --- | --- |
| Linux x86_64 | `x86_64-unknown-linux-musl` | Release target; validation pending | Musl validation pending |
| Linux arm64 | `aarch64-unknown-linux-musl` | Release target | Not yet supported |
| Windows x86_64 | `x86_64-pc-windows-msvc` | Release target | CI build/run configured; native validation pending |
| Windows arm64 | `aarch64-pc-windows-msvc` | Release target | CI build/run configured; native validation pending |
| macOS arm64 | `aarch64-apple-darwin` | Release target | Not yet supported |

These are configured targets, not equivalent validation claims. Recorded native
Slang-only evidence is Linux x86_64/glibc; local musl execution remains pending.

CI tests and builds all targets above on pushes to `master`, or when you select
a branch under **Actions → CI and Release → Run workflow**. Publishing a release
in the GitHub web UI runs the same checks for its tag and attaches binary
packages to that existing release after all jobs pass. Saving a draft does not
trigger CI. For a tag such as `v1.2.3`, download packages use
`lapligence-1.2.3-<os>-<arch>.<ext>`: `os` is `linux`, `windows`, or `macos`,
and `arch` is `x64` or `arm64` for the targets above. Windows packages are ZIP
files; Linux and macOS packages are `.tar.gz` files. Each includes `llg`,
`llg_ls` (with `.exe` on Windows), `readme.md`, and `LICENSE`. SHA-256 checksum
files accompany the packages.

The generated simulator uses a bundled pure-C11 stackless coroutine runtime.
The release workflow configures generated-model build/run checks for MSVC x64
and ARM64; those lanes remain unvalidated until native results are recorded.
Other table entries likewise remain configuration, not support claims.

## Build prerequisites

All platforms require:

- A recursive Git checkout: `git submodule update --init --recursive`.
- Rust and Cargo 1.98.0 from [rustup](https://rustup.rs/), as pinned in
  [`rust-toolchain.toml`](rust-toolchain.toml).
- CMake 3.20 or newer.
- A C and C++ compiler with the platform's standard build tools.
- Python 3 for Slang's syntax and diagnostic generators.
- `patch`, or Git with `git apply` support.

Waveform support needs no system zlib. `llg` embeds the zlib sources it needs
from the `vendor/zlib` submodule (v1.3.2) and compiles them, with prefixed
symbols, only into generated models that use waveform tasks.

`Cargo.lock` pins the Rust dependency graph. Use Cargo's `--locked` option for
reproducible builds and tests; the root commit's gitlink pins `vendor/slang`.
The serialized U05 baseline and per-patch workflow is
documented in [`tests/readme.md`](tests/readme.md).

### Linux

On Ubuntu or Debian, install the native prerequisites:

```sh
sudo apt-get update
sudo apt-get install build-essential cmake patch python3
```

Build both programs:

```sh
cargo build --locked --release --bin llg --bin llg_ls
```

For a fully static Linux x86_64 build, the included Docker image provides the
musl toolchain and native dependencies:

```sh
docker build --platform linux/amd64 --build-arg UID=$(id -u) \
  --build-arg GID=$(id -g) -t llg-dev .
mkdir -p target
docker run --rm --platform linux/amd64 -v "$(pwd)":/workspace \
  llg-dev cargo build --locked --release --bin llg --bin llg_ls \
  --target x86_64-unknown-linux-musl
```

The image uses the supplied non-root UID/GID. Create the mounted `target` as your
host user; repair root-owned output with
`sudo chown -R "$(id -u):$(id -g)" target`. Rebuild the image when its user differs.

### macOS arm64

- Install Xcode Command Line Tools: `xcode-select --install`.
- Install CMake and Python 3 with your package manager.
- Add and build the Rust target:

```sh
rustup target add aarch64-apple-darwin
cargo build --locked --release --bin llg --bin llg_ls \
  --target aarch64-apple-darwin
```

### Windows

- Install Visual Studio Build Tools with the **Desktop development with C++**
  workload and the Windows SDK.
- Install CMake, Python 3, Git, and Rust 1.98.0.
- Run the build from a matching MSVC Developer PowerShell.

Build for x86_64:

```powershell
rustup target add x86_64-pc-windows-msvc
cargo build --locked --release --bin llg --bin llg_ls `
  --target x86_64-pc-windows-msvc
```

For Windows arm64, install the MSVC arm64 tools and replace the target with
`aarch64-pc-windows-msvc`.

## Optional development accelerators

Sccache for Rust compilation and mold for linking are **off by default**. Install
the tools you want on `PATH`; mold requires a Linux GNU Rust host and a C compiler.
From the repository root, enable either or both for plain Cargo in Bash:

```bash
export LLG_SCCACHE=1 LLG_MOLD=1; source scripts/dev-env.sh
cargo build --locked --profile quick --bin llg -v
```

Omit either environment variable to enable only the other tool. For the test
runner, use `scripts/run-tests.sh --cargo-profile quick --test sim_function
--sccache --mold` (omit either flag as needed). Requested tools must be installed;
missing tools produce an error. An existing `RUSTC_WRAPPER` is preserved.

In verbose compiler commands, check for `scripts/sccache.sh` and
`-C linker=.../scripts/mold-linker.sh`; `sccache --show-stats` reports cache usage.
On Linux, `readelf --string-dump .comment target/quick/llg` should identify mold.
See [development accelerators](tests/readme.md#optional-development-accelerators)
for native C/C++ caching, platform limits, and thread controls.

## Using `llg`

```text
llg [options] [<file.sv>...] [-- <plusargs>...]
```

Options can also come from an `llg.toml`: `./llg.toml` is read when present, or
the file named by `--config <file>` (which must exist). The same file configures
`llg_ls`. Command-line values override the file, a repeatable option on the
command line replaces the file's whole list, and files named on the command line
replace the file's sources; see [configuration](docs/config.md) for every key
and the precedence. Config errors name the file and key and exit 1.

Common options:

- `--config <file>`: read this `llg.toml` instead of `./llg.toml`.
- `--param-override <NAME=VALUE>` / `-G`: override a top-level parameter.
- `--no-lint`, `--no-gen-only`, `--opt`: undo `lint.run`, `build.gen_only` and
  `simulator.optimize = false` from the config file.

- `--top <module>`: select the top-level module.
- `--edition <2001|2009>`: select the compilation language edition (default `2009`).
- `--compilation-units <separate|merged>`: select per-file or shared
  preprocessing and `$unit` scope (default `separate`).
- `--include-dir <path>` / `-I <path>`: add a bounded, canonical include root.
- `--define <NAME[=VALUE]>` / `-D <NAME[=VALUE]>`: seed preprocessing macros.
- `--lint`: lint before simulation; lint errors stop the build.
- `--lint-json [<path>]`: write a JSON lint report and exit.
- `--lint-config <file>`: load rule settings from a TOML file.
- `--gen-only`: generate C11 sources and `CMakeLists.txt` without building.
- `--max-export-mib <MiB>`: bound frontend snapshot export for the elaborated
  design (default 4096 MiB; range 1–16384). An exhausted budget reports its
  effective limit. Export bytes cover captured records and strings, not total
  process memory; use `LLG_MEMORY_LIMIT_MB` for the optional process-wide guard.
- `--out-dir <dir>`: output root (default `build`). The model is written to
  `<dir>/sim/<design>`; its executable is `<dir>/sim/<design>/build/bin/sim`.
- `--runtime-cache <dir>`: runtime archive cache (default
  `<out-dir>/llg-runtime-cache`).
- `--cc <program>`: C compiler for the model (default `cc`).
- `--model-opt-level <O0|O1|O2|O3|Os>`: model and runtime C optimization
  (default O3, retained with `--gen-only`). MSVC maps O0 to `/Od`, O1/Os to
  `/O1`, and O2/O3 to `/O2`.
- `--cflags <flags>`: extra C compiler flags, appended after the selected
  optimization level so user optimization flags override it. The option
  replaces `LLG_CFLAGS`; an empty string suppresses the environment flags.
- `--build-jobs <N>`: parallel model/runtime compile jobs (positive integer;
  otherwise `CMAKE_BUILD_PARALLEL_LEVEL`, then available CPUs).
- `--cmake <program>`: CMake program (default `cmake`).
- `--generator <name>`: choose a CMake generator, such as `Ninja`.
- `--launcher <program>`: optionally set CMake's C compiler launcher, such as
  `ccache` or `sccache`; no launcher is selected by default.
- `--`: pass the remaining arguments to the generated simulator for
  `$test$plusargs`/`$value$plusargs` (for example, `llg tb.sv -- +mode=fast`).

Relative paths resolve from the current directory. The runtime cache holds
compiled runtime archives shared by compatible models, so only model-specific C
is compiled per design. `--gen-only` output is self-contained and does not use
the cache. Release contributes only NDEBUG; it does not add another
optimization level. Compiler flags and the selected level identify compatible
runtime archives.

Exit status is `0` on success, `1` on compile/lint/build errors, and `2` for
invalid command-line usage. A completed simulator's exit status is propagated.

### Simulation example

Create `hello.sv`:

```systemverilog
module hello;
  initial begin
    $display("Hello from Lapligence");
    $finish;
  end
endmodule
```

Build and run it:

```sh
target/release/llg --top hello hello.sv
```

The generated sources and executable are written under `build/sim/hello/`.

Other useful invocations:

```sh
# Lint and then simulate.
target/release/llg --lint --top hello hello.sv

# Produce a machine-readable lint report without simulating.
target/release/llg --lint-json lint-results.json hello.sv

# Generate model sources without building or running them.
target/release/llg --gen-only --top hello hello.sv
```

To generate waveforms, use `$dumpfile("trace.vcd")` or
`$dumpfile("trace.fst")` with `$dumpvars` in the HDL source.

### Rerunning a built model

The simulator executable can be run again without `llg`. Run-time environment
variables choose where each run writes its files:

```sh
target/release/llg --top tb tb.sv      # build build/sim/tb and run once
LLG_SIM_OUT_DIR=runs/a LLG_SIM_LOG_FILE=sim.log build/sim/tb/build/bin/sim +seed=1
LLG_SIM_OUT_DIR=runs/b LLG_SIM_WAVE_FILE=wave.fst build/sim/tb/build/bin/sim +seed=2
```

### Environment variables

Command-line options take precedence over the matching variable.

Model build (`llg`):

| Variable | Effect |
| --- | --- |
| `LLG_RUNTIME_CACHE_DIR` | Runtime cache when `--runtime-cache` is not given. |
| `LLG_CC`, then `CC` | C compiler when `--cc` is not given. |
| `LLG_CFLAGS` | Extra C flags when `--cflags` is not given. |
| `LLG_CMAKE` | CMake program when `--cmake` is not given. |
| `CMAKE_GENERATOR` | CMake generator when `--generator` is not given. |

Simulation run time (read by the simulator executable; `llg` passes its
environment through):

| Variable | Effect |
| --- | --- |
| `LLG_SIM_OUT_DIR` | Directory for relative files the simulation writes: the waveform, `$fopen` in write/append mode, `$writememh`/`$writememb` and `LLG_SIM_LOG_FILE`. Created if missing. Files the simulation reads stay relative to the current directory. |
| `LLG_SIM_WAVE_FILE` | Waveform file (`.vcd` or `.fst`). Replaces the `$dumpfile` name and the default `dump.vcd`. |
| `LLG_SIM_LOG_FILE` | Copy of stdout and stderr (Linux and macOS). |
| `LLG_STOP_POLICY` | `$stop` handling, `resume` (default) or `exit`. `llg` sets it from `--stop-policy`. |
| `LLG_ZERO_LOOP_LIMIT` | Maximum region passes at one time step (default 10000000). |
| `LLG_PROCESS_STEP_LIMIT` | Maximum loop steps one process may run at one time step (default 10000000). |
| `LLG_ALLOW_SYSTEM` | `1`, `true`, `yes` or `on` enables `$system`. |
| `LLG_VPI_PLUGIN` | VPI plugin libraries to load, separated by `:` (`;` on Windows). Plugins resolve `vpi_*` from the model at load time; on macOS link them with `-undefined dynamic_lookup`. |

Both `llg` and `llg_ls`:

| Variable | Effect |
| --- | --- |
| `LLG_MEMORY_LIMIT_MB` | Memory budget in MiB for the frontend process; unset disables it. |
| `LLG_MEMORY_WARNING_PERCENT` | Warn at this percentage of the budget (default 80). |
| `LLG_MEMORY_POLL_MS` | Memory sampling interval in milliseconds (default 1000). |
| `LLG_MEMORY_ADDRESS_SPACE_LIMIT` | `true` also applies the budget as a Linux/macOS address-space limit. |

## Using `llg_ls`

`llg_ls` is a stdio LSP server. An editor or LSP client should launch it as a
child process; it is not an interactive terminal program.

Use the release binary as the editor's server command:

```text
/absolute/path/to/lapligence/target/release/llg_ls
```

Each workspace can contain an `llg.toml`:

```toml
schema_version = 1

[sources]
directories = ["rtl", "tb"]
include = ["**/*.v", "**/*.sv"]

[compile]
top = "tb"
include_dirs = ["include"]
defines = ["SIMULATION"]

[lint.rules.width-mismatch]
severity = "error"
```

- Missing configuration uses the workspace root and discovers `.v`/`.sv`
  files recursively.
- Configuration changes are reloaded without restarting the server.
- Logs go to stderr and never corrupt the stdio protocol.
- Set `LLG_LOG=debug` for diagnostic logging or `LLG_LOG_FILE=<path>` to write
  logs to a file.
- Unsaved editor buffers are staged under `--staging-dir <dir>`, or the OS temp
  directory (`TMPDIR`) by default. Each server uses its own subdirectory and
  removes it on exit. Keep the staging directory outside workspace roots.

Options: `--stdio` (default mode), `--staging-dir <dir>`, `--dump-tokens <path>`
(print token bindings for a file or directory and exit).

See [configuration](docs/config.md) for all source, compile, analysis, and
lint settings. The file is shared with `llg`; keys only the simulator driver
uses (`[build]`, `[simulator]`, `[output]`, `[libraries]`, ...) are accepted and
ignored by the server.

## Running tests

```sh
cargo install cargo-nextest --locked
scripts/run-tests.sh                         # complete suite
scripts/run-tests.sh --test sim_counter      # one integration suite
scripts/run-tests.sh --test-work-dir /build      # opt-in tmpfs for test scratch/cache
cargo nextest run --locked --all-features    # direct invocation
```

Nextest defaults to 8 concurrent tests; a large host can opt into
`cargo nextest run --locked --profile max-threads --all-features` for 32.
Before submitting, run the full [repository gate](tests/readme.md#repository-gate),
including checks with/without LSP, clippy, tests and doctests.

## Build cleanup

Native frontend and generated-model builds can consume significant disk space:

```sh
scripts/clean.sh --dry-run  # show removable build data
scripts/clean.sh            # remove old frontend and simulator outputs
scripts/clean.sh --all      # also remove the active frontend build
```

## Source organization

See [the source layout](docs/source_layout.md) for the Rust facade/domain map,
private C runtime fragments, and test ownership. Use
[Coding Practices](docs/coding_practices.md) when extending these boundaries.


## Dynamic ownership validation

See [dynamic ownership validation](tests/readme.md#dynamic-ownership-validation)
for component, generated-model and HDL checks. Component success does not establish
full simulator or platform acceptance.
