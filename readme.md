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
| Windows x86_64 | `x86_64-pc-windows-msvc` | Release target | Not yet supported |
| Windows arm64 | `aarch64-pc-windows-msvc` | Release target | Not yet supported |
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

The generated simulator uses a bundled coroutine runtime that is currently
x86/Unix-only. On arm64 and Windows, `llg` can still lint, elaborate, and emit
C with `--gen-only`, but it cannot build and run that emitted model yet.

## Build prerequisites

All platforms require:

- A recursive Git checkout: `git submodule update --init --recursive`.
- Rust and Cargo 1.98.0 from [rustup](https://rustup.rs/), as pinned in
  [`rust-toolchain.toml`](rust-toolchain.toml).
- CMake 3.20 or newer.
- A C and C++ compiler with the platform's standard build tools.
- Python 3 for Slang's syntax and diagnostic generators.
- `patch`, or Git with `git apply` support.
- zlib development files for waveform-enabled generated models.

`Cargo.lock` pins the Rust dependency graph. Use Cargo's `--locked` option for
reproducible builds and tests; the root commit's gitlinks pin `vendor/slang`
and `vendor/libaco`. The serialized U05 baseline and per-patch workflow is
documented in [`tests/readme.md`](tests/readme.md).

### Linux

On Ubuntu or Debian, install the native prerequisites:

```sh
sudo apt-get update
sudo apt-get install build-essential cmake patch python3 zlib1g-dev
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
- Install CMake and Python 3 with your package manager. The Xcode SDK supplies
  zlib.
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

## Using `llg`

```text
llg [options] <file.sv>... [-- <plusargs>...]
```

Common options:

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
- `--generator <name>`: choose a CMake generator, such as `Ninja`.
- `--launcher <program>`: optionally set CMake's C compiler launcher, such as
  `ccache` or `sccache`; no launcher is selected by default.
- `--`: pass the remaining arguments to the generated simulator for
  `$test$plusargs`/`$value$plusargs` (for example, `llg tb.sv -- +mode=fast`).

Compatible runtime archives are cached in `target/llg-runtime-cache`, leaving
model-specific C to compile. `LLG_RUNTIME_CACHE_DIR` overrides the cache (relative
to repository root). `--gen-only` output is self-contained and cache-independent.

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

The generated sources and executable are written under `target/sim/hello/`.

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

See [LSP configuration](docs/config.md) for all source, compile, analysis, and
lint settings.

## Running tests

```sh
cargo install cargo-nextest --locked
scripts/run-tests.sh                         # complete suite
scripts/run-tests.sh --test sim_counter      # one integration suite
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
