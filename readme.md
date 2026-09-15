# Lapligence (`llg`)

Lapligence provides Verilog/SystemVerilog simulation and editor tooling through
two programs:

- **`llg`** compiles, lints, builds, and runs Verilog/SystemVerilog simulations.
- **`llg_ls`** provides Language Server Protocol (LSP) features to editors.

## What it can do

### Simulator and linter: `llg`

- Compile and elaborate Verilog/SystemVerilog designs.
- Run 24 built-in lint rules, with configurable severities.
- Generate an optimized C11 simulation model.
- Build and run the generated model automatically with CMake.
- Produce VCD or FST waveforms from standard `$dump*` system tasks.
- Emit lint results as readable diagnostics or JSON.
- Stop after source generation for integration with another build flow.

### Language server: `llg_ls`

- Publish compiler and lint diagnostics.
- Provide semantic highlighting, hover information, and completion.
- Find definitions and references.
- Provide document and workspace symbols.
- Support rename and prepare-rename requests.
- Analyze multiple workspace roots independently.
- Track unsaved editor buffers and configuration changes.
- Expose a read-only module and instance hierarchy explorer.

## Simplified workflow

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

The first build also compiles the vendored HDL frontend and can take several
minutes. Its native build artifacts can use several gigabytes.

## Supported HDL features

The simulator currently covers a practical RTL-oriented subset, including:

- Verilog and SystemVerilog modules, ports, parameters, and generate blocks.
- Four-state values, packed vectors, arrays, memories, strings, and scalar
  real/shortreal values.
- `initial`, `always`, `always_comb`, `always_ff`, tasks, and functions.
- Blocking and non-blocking assignments, event controls, delays, and `#0`/NBA
  scheduling.
- Continuous assignments, common logic and tri-state gates, and basic net
  resolution.
- Hierarchical reads and writes, interfaces, and module instance arrays.
- `$display`, `$monitor`, `$strobe`, `$time`, `$finish`, and waveform tasks.

Lapligence does not yet implement the complete Verilog/SystemVerilog language.
See [simulator feature status](docs/sim_features.md) for tested features and
known limitations. See [lint rules](src/core/lint/readme.md) for the rule list.

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

The matrix lists configured release targets, not equivalent validation claims.
The Slang-only pipeline has native test evidence on Linux x86_64 with glibc.
Musl build support has been source-reviewed but has not been run locally.

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

The image runs as a non-root user with the UID/GID supplied at image build
time. The repository mount already includes `target`; create it as your host
user before running Docker. If an earlier run created root-owned build output,
restore its ownership with `sudo chown -R "$(id -u):$(id -g)" target`.
Rebuild the image with the arguments above if its user does not match yours.

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

Normal builds cache compatible C runtime archives under
`target/llg-runtime-cache` in the repository, so generated models usually
compile only their model-specific C file. Set `LLG_RUNTIME_CACHE_DIR` to choose
a different cache root; relative override paths are resolved from the
repository root. `--gen-only` output remains self-contained and does not require
that cache.

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

Run the complete suite:

```sh
scripts/run-tests.sh
```

Run one integration-test binary:

```sh
scripts/run-tests.sh --test sim_counter
```

The script uses `cargo-nextest`, which is the repository's default test
runner. Install it with:

```sh
cargo install cargo-nextest --locked
```

Run tests directly through nextest:

```sh
cargo nextest run --locked --all-features
```

The default profile runs up to 8 tests concurrently. On a machine with enough
CPU and memory, opt in to the 32-thread profile:

```sh
cargo nextest run --locked --profile max-threads --all-features
```

Before submitting a change, run the main checks:

```sh
cargo fmt --check
cargo check --locked --all-targets --all-features
cargo check --locked --lib --no-default-features
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo nextest run --locked --all-features
cargo test --locked --doc --all-features
```

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
