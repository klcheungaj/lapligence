# Lapligence (llg)

Lapligence is a Verilog/SystemVerilog **simulator** and **language server**, built on
Surelog + UHDM.

Pipeline:
`Verilog → Surelog → UHDM → core::db → IR → optimization → C11 emission → CMake build → run`

## Requirements

- Rust (stable) with cargo
- C/C++ toolchain: `cc`, `cmake` (the only model builder), `make`
- Linux or macOS; static musl builds supported (`x86_64-unknown-linux-musl`)

First build compiles the vendored Surelog/UHDM/ANTLR — expect it to be slow.

## Build

```sh
cargo build --bin llg_ls     # language server
cargo build --bin llg        # simulator driver
cargo build --release         # everything, optimized
```

Docker alternative (see `Dockerfile`):

```sh
docker build --build-arg UID=$(id -u) --build-arg GID=$(id -g) -t llg-dev .
mkdir -p target
docker run -t --rm \
  -v "$(pwd)":/workspace \
  -v "$(realpath target)":/workspace/target \
  -u $(id -u):$(id -g) \
  llg-dev \
  cargo build --release --target x86_64-unknown-linux-musl
```

### Disk cleanup

Artifacts accumulate across profiles and target triples: `target/surelog/`
holds a full Surelog/UHDM C++ build per `<triple>/<profile>` (gigabytes each),
every simulator model lands in `target/sim/<design>/`, and Surelog drops
`slpp_all/` into whatever working directory a manual run used.  Simulator
rebuilds clean their own output directory (stale generated files are pruned;
CMake trees whose cached generator no longer matches are rebuilt from
scratch); for the rest run:

```sh
scripts/clean.sh --dry-run   # list what would be removed, with sizes
scripts/clean.sh             # remove non-selected target/surelog builds,
                             # target/sim model outputs, and slpp_all/
scripts/clean.sh --all       # also remove the currently selected surelog tree
                             # (forces a full Surelog rebuild next build)
```

## Binaries

| Binary | Purpose |
|---|---|
| `llg_ls` | Verilog/SV Language Server (stdio JSON-RPC) |
| `llg` | Simulator driver: compile → optimize → emit C → build → run |
| `elab_check` | Elaboration verifier (instance tree, ref binding, params) |
| `helloworld` / `hellouhdm` / `llg_demo` | Raw Surelog/UHDM API demos |

## Language-server safeguards and tracing

The LSP applies per-root `[analysis]` limits of 1 MiB per unique input file
and 8 MiB across unique source/include inputs by default. Override them in
`llg.toml` when a project needs larger files. The process-wide memory safeguard
is opt-in through `LLG_MEMORY_LIMIT_MB`; it measures the complete `llg_ls` or
`llg` process and terminates immediately at the configured physical-memory
ceiling. See [configuration](docs/config.md) for the per-root input limits.

For request-to-Surelog diagnostics, run with `LLG_LOG=debug` (or `trace` for
every transport message) and optionally set `LLG_LOG_FILE`. Logs always go to
stderr/the configured file, never stdout, so stdio JSON-RPC remains valid.

Open-buffer semantic tokens are served from the current unsaved text only
when that text has no syntax diagnostics; incomplete syntax returns an empty
token stream instead of unstable partial highlighting or stale cached colors.
Inactive conditional branches and non-lexical compiler-directive lines are
position-preservingly masked for this isolated parse so valid directives such
as `` `include`` do not become false syntax errors, and include contents are
not read by the request.
The custom `llg/moduleExplorer` response is bounded as well, but reserves
capacity for useful hierarchy roots and module definitions before optional
port/parameter/signal contents can consume the remaining response budget.

## Using the simulator (`llg`)

```sh
llg [generate options] [build options] [--top <module>] <file.sv>...
```

Generate-time options:

- `--top <module>` — top module to elaborate
- `--lint` — run the linter before codegen; lint errors abort (exit 1)
- `--lint-json [<path>]` — machine-readable lint report only, no simulation
- `--lint-config <file>` — load a `llg-lint.toml` rule configuration
- `--gen-only` — emit model + runtime + `CMakeLists.txt` into
  `target/sim/<design>` (prints the directory), then exit without
  configuring/building/running

The 24-rule shared registry is also used by the language server, including
checks for undriven signals, incomplete sensitivity lists, out-of-range
selects, X/Z logical equality, duplicate exact-case labels, empty implicit
sensitivity, assignment expressions in conditions, and `casex`. See
[`src/core/lint/readme.md`](src/core/lint/readme.md) for rule and configuration
details.

Build-time options:

- `--generator <backend>` — cmake `-G` backend (e.g. `Ninja`,
  `"Unix Makefiles"`); overrides `$CMAKE_GENERATOR`

CMake is the only supported model builder; it runs automatically after C
emission. Exit codes: `0` success · `1` errors · `2` usage error. The
simulator's exit code is propagated.

Waveforms are enabled from HDL with `$dumpfile("trace.vcd")` or
`$dumpfile("trace.fst")` plus `$dumpvars`. Waveform-enabled generated models
need zlib and a thread library discoverable by CMake; all GTKWave libfst source
is emitted from the repository, with no build-time download. A dedicated
writer thread consumes a bounded lossless ring so the simulation scheduler
does not perform compression or file I/O. `$dumpon`, `$dumpoff`, `$dumpall`,
`$dumpflush`, and `$dumplimit` are supported; `$dumpvars` depth/scope filtering
is not yet implemented and currently dumps all user-visible storage with a
warning.

Environment variables:

- `LLG_CC` / `CC` — C compiler for the generated model
- `LLG_CFLAGS` — extra compiler flags (e.g. sanitizers)
- `LLG_CMAKE` — override the cmake program
- `CMAKE_GENERATOR` — passed through to cmake

### Example

```systemverilog
// tb.sv
module counter(clk, rst, out);
  input clk, rst;
  output reg [7:0] out;
  always @(posedge clk) out <= rst ? 8'd0 : out + 8'd1;
endmodule

module tb;
  reg clk = 0, rst = 1;
  wire [7:0] cnt;
  counter u(.clk(clk), .rst(rst), .out(cnt));
  always #5 clk = ~clk;
  initial begin
    #12 rst = 0;
    #50 $display("cnt=%0d", cnt);
    $finish;
  end
endmodule
```

Run it:

```sh
$ llg --top tb tb.sv
cnt=5
```

Artifacts land in `target/sim/tb/`: the emitted C model, runtime sources,
and the CMake-built executable under `build/bin/`.

## Tests

```sh
scripts/run-tests.sh                          # whole suite, parallel
scripts/run-tests.sh --test sim_counter       # subset; args pass through
cargo test                                    # plain libtest, always available
```

`scripts/run-tests.sh` uses [cargo-nextest](https://nexte.st) when installed
(`cargo install cargo-nextest --locked`) and falls back to `cargo test`
otherwise.  nextest runs every test in its own process, so the many
integration-test binaries execute concurrently instead of one-by-one; the
profile lives in `.config/nextest.toml` (no retries; Surelog-heavy suites are
grouped into a capped-concurrency `surelog-heavy` pool).

Parallelism is safe by construction: suites that move the process-global
working directory serialize their tests internally through a mutex, and every
temporary directory is keyed by `std::process::id()` (plus a per-test counter),
so concurrently running binaries never share paths.  Plain `cargo test`
remains supported and runs tests within each binary on parallel threads.
