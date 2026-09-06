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
 Parse and elaborate (using Surelog)
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
| Linux x86_64 | `x86_64-unknown-linux-musl` | Supported | Supported |
| Linux arm64 | `aarch64-unknown-linux-musl` | Release target | Not yet supported |
| Windows x86_64 | `x86_64-pc-windows-msvc` | Release target | Not yet supported |
| Windows arm64 | `aarch64-pc-windows-msvc` | Release target | Not yet supported |
| macOS arm64 | `aarch64-apple-darwin` | Release target | Not yet supported |

The matrix lists configured release targets, not equivalent validation claims.
Full native-run evidence is currently recorded only for Linux x86_64.

The generated simulator uses a bundled coroutine runtime that is currently
x86/Unix-only. On arm64 and Windows, `llg` can still lint, elaborate, and emit
C with `--gen-only`, but it cannot build and run that emitted model yet.

## Build prerequisites

All platforms require:

- A recursive Git checkout: `git submodule update --init --recursive`.
- Stable Rust and Cargo from [rustup](https://rustup.rs/).
- CMake 3.20 or newer.
- A C and C++ compiler with the platform's standard build tools.
- Python 3 with the `orderedmultidict` package.
- A Java 11 or newer runtime for the parser generator.
- `patch`, or Git with `git apply` support.
- zlib development files for waveform-enabled generated models.

A Python virtual environment keeps the build dependency local:

```sh
python3 -m venv .venv
. .venv/bin/activate
python -m pip install orderedmultidict
```

### Linux

On Ubuntu or Debian, install the native prerequisites:

```sh
sudo apt-get update
sudo apt-get install build-essential cmake patch python3 python3-venv \
  default-jre zlib1g-dev
```

Build both programs:

```sh
cargo build --release --bin llg --bin llg_ls
```

For a fully static Linux x86_64 build, the included Docker image provides the
musl toolchain and native dependencies:

```sh
docker build --platform linux/amd64 --build-arg UID=$(id -u) \
  --build-arg GID=$(id -g) -t llg-dev .
docker run --rm --platform linux/amd64 -v "$(pwd)":/workspace \
  -v llg-target:/workspace/target \
  llg-dev cargo build --release --bin llg --bin llg_ls \
  --target x86_64-unknown-linux-musl
```

### macOS arm64

- Install Xcode Command Line Tools: `xcode-select --install`.
- Install CMake, Python 3, and Java 11+ with your package manager. The Xcode
  SDK supplies zlib.
- Create the Python environment shown above.
- Add and build the Rust target:

```sh
rustup target add aarch64-apple-darwin
cargo build --release --bin llg --bin llg_ls \
  --target aarch64-apple-darwin
```

### Windows

- Install Visual Studio Build Tools with the **Desktop development with C++**
  workload and the Windows SDK.
- Install CMake, Python 3, Java 11+, Git, and stable Rust.
- Run the build from a matching MSVC Developer PowerShell.
- Create and activate the Python environment:

```powershell
py -m venv .venv
.\.venv\Scripts\Activate.ps1
python -m pip install orderedmultidict
```

Build for x86_64:

```powershell
rustup target add x86_64-pc-windows-msvc
cargo build --release --bin llg --bin llg_ls `
  --target x86_64-pc-windows-msvc
```

For Windows arm64, install the MSVC arm64 tools and replace the target with
`aarch64-pc-windows-msvc`.

## Using `llg`

```text
llg [options] <file.sv>...
```

Common options:

- `--top <module>`: select the top-level module.
- `--lint`: lint before simulation; lint errors stop the build.
- `--lint-json [<path>]`: write a JSON lint report and exit.
- `--lint-config <file>`: load rule settings from a TOML file.
- `--gen-only`: generate C11 sources and `CMakeLists.txt` without building.
- `--generator <name>`: choose a CMake generator, such as `Ninja`.

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

The script uses `cargo-nextest` when available and otherwise falls back to
`cargo test`. Install nextest with:

```sh
cargo install cargo-nextest --locked
```

Run the same serialized test command used by CI:

```sh
cargo test --all-features -- --test-threads=1
```

Before submitting a change, run the main checks:

```sh
cargo fmt --check
cargo check --all-targets --all-features
cargo check --lib --no-default-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features -- --test-threads=1
```

## Build cleanup

Native frontend and generated-model builds can consume significant disk space:

```sh
scripts/clean.sh --dry-run  # show removable build data
scripts/clean.sh            # remove old frontend and simulator outputs
scripts/clean.sh --all      # also remove the active frontend build
```
