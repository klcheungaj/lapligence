# Verilog/SystemVerilog simulator

The simulator lowers owned frontend data into a validated execution model, emits
C11 and builds it with CMake. Generated models link their own stackless runtime;
it is not a Rust runtime dependency.

| Component | Responsibility |
| --- | --- |
| `semantic/` | Frontend-neutral model, source origins, synthesis classification and executable-node coverage. |
| `codegen/` | Typed lowering of declarations, values, processes, links and scheduling. |
| `ir/` | Shared typed storage/value/statement tables and validation. |
| `execution/` | Executable blocks, effects, regions and suspend/resume plans. |
| `opt/` | Shape- and four-state-aware execution-model optimizations. |
| `emit_c/` | Ordered C setup/evaluation/cleanup and model lifecycle. |
| `rt/` | Exact-width values, scheduler, native services and optional waveform output. |

`generate_from_db_with_opts` reuses one owned DB across optimization variants.
`GeneratedModel` carries the design name, generated C and warnings. Unsupported
executable forms fail with source-linked errors before C compilation.

See [feature status](../../docs/sim_features.md),
[build and CLI usage](../../readme.md), [tests](../../tests/readme.md) and
[source layout](../../docs/source_layout.md).
