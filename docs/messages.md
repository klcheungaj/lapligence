# llg messages

<!-- Generated from src/core/report/catalog.rs and src/sim/rt/llg_msg_ids.def by `cargo test --test general messages_catalog::regenerate -- --ignored`; do not edit. -->

Every message `llg` and a generated model print on stderr has one layout and a stable id. Design output (`$display` and friends) and the user text of `$info`/`$warning`/`$error`/`$fatal` are never changed; streams and exit codes are described in the [CLI readme](../src/bin/readme.md).

## Layout

The layout is `src/sim/rt/llg_msg_layout.def`, read by the host and the model runtime:

```text
{severity}: <{location}: >[{id}] <({context}) >{text}< ({hint})>
```

Examples:

```text
WARNING: f.sv:7:9: [LINT-001] signal `x` in `tb` is never used (unused-signal)
ERROR: [CLI-003] --top requires a module name
INFO: f.sv:3:35: [SIM-001] (tb @ 5 ns) $finish called
```

Severity words: `FATAL`, `ERROR`, `WARNING`, `INFO`. The id is the tool name, `-` and a three-digit index; index 000 of a tool is its summary. The location is the physical `file:line:col`, followed by `` (`line file:line)`` when a `` `line`` directive maps it, and is left out (with its separator) when a message has none. Design context is the hierarchical scope and the simulation time, joined by ` @ `; the time is in the largest unit that shows it exactly. The hint, when present, names the lint rule or frontend warning option that selects the message.

Each phase that reported a warning or worse ends with a summary message, `LINT-000` for lint, with the text `<{fatal} fatal, >{error} error(s), {warning} warning(s)<, {info} info(s)>`.

## Tools

| Tool | Reports |
|---|---|
| `CLI` | command-line arguments of `llg` |
| `CONFIG` | the `--config` file and environment settings |
| `HOST` | the `llg` process itself (files it writes, memory guard) |
| `SLANG` | frontend diagnostics from Slang (parse, elaboration, type checks) |
| `ELAB` | llg checks of the elaborated design before lint and code generation |
| `LINT` | lint rules |
| `CODEGEN` | lowering and C code generation |
| `OPT` | the optimizer (reserved: it reports nothing today) |
| `BUILD` | CMake and the C compiler building the model |
| `RUN` | how the model process ended, seen by `llg` |
| `SIM` | the generated model at run time |

## CLI

| Id | Severity | Meaning | Disable |
|---|---|---|---|
| `CLI-001` | error | the simulator driver does not run on this platform (exit 2) | - |
| `CLI-002` | error | an unknown `-` option (exit 2) | - |
| `CLI-003` | error | an option's value is missing or invalid (exit 2) | - |
| `CLI-004` | error | no source files were named (exit 2) | - |
| `CLI-005` | warning | build options are ignored with `--gen-only` | leave the build options out |

## CONFIG

| Id | Severity | Meaning | Disable |
|---|---|---|---|
| `CONFIG-001` | error | the `--config` file does not exist or cannot be read (exit 1) | - |
| `CONFIG-002` | error | a `--config` key or value is invalid; one message per error (exit 1) | - |
| `CONFIG-003` | warning | a `--config` key is accepted but has no effect for `llg` | remove the key |
| `CONFIG-004` | error | an environment setting is invalid (exit 1) | - |

## HOST

| Id | Severity | Meaning | Disable |
|---|---|---|---|
| `HOST-001` | error | `llg` cannot write an output file (lint JSON, import library) | - |
| `HOST-002` | warning | `llg` cannot write an optional file (an `$sdf_annotate` log) | - |
| `HOST-003` | warning | process memory guard warning | unset `LLG_MEMORY_LIMIT_MB` |
| `HOST-004` | info | process memory guard status | unset `LLG_MEMORY_LIMIT_MB` |

## SLANG

Frontend diagnostics keep Slang's text; the id is `SLANG-<name>` with the diagnostic name from Slang's list (`vendor/slang/scripts/diagnostics.txt`), and the hint is the warning option (`-W<option>`) when Slang has one. Slang notes print as `INFO`. A frontend warning is disabled in the source with `` `pragma diagnostic ignore="-W<option>"`` (IEEE 1800-2009 22.11 leaves pragma meaning to the tool).

| Id | Severity | Meaning | Disable |
|---|---|---|---|
| `SLANG-000` | info | end-of-phase counts, printed when the phase reported a warning or worse | - |

## ELAB

| Id | Severity | Meaning | Disable |
|---|---|---|---|
| `ELAB-000` | info | end-of-phase counts, printed when the phase reported a warning or worse | - |
| `ELAB-001` | error | the frontend could not start (sources unreadable, limits) | - |
| `ELAB-002` | error | the owned design database could not be built from the frontend snapshot | - |
| `ELAB-003` | error | protected source envelopes are rejected by design | - |
| `ELAB-004` | error | a construct outside the selected `--edition` profile | select another `--edition` |
| `ELAB-010` | error | rejected by design: legacy procedural assign/deassign form | - |
| `ELAB-011` | error | rejected by design: MOS and resistive switch primitives | - |
| `ELAB-012` | error | rejected by design: trireg charge storage | - |
| `ELAB-013` | error | rejected by design: charge and delay-mode directives | - |
| `ELAB-014` | error | rejected by design: extended VCD port dumping | - |
| `ELAB-015` | error | rejected by design: stochastic queue form | - |
| `ELAB-016` | error | rejected by design: legacy PLA tasks | - |
| `ELAB-017` | error | rejected by design: legacy driver and scope inspection tasks | - |
| `ELAB-018` | error | rejected by design: PLI 1.0 TF/ACC interface | - |

## LINT

The hint is the rule name. Disable or re-grade a rule with `[lint.rules.<rule>] enabled = false` or `severity = "error"|"warning"|"info"` in the `--config` file, or every rule with `[lint] enabled = false`; `-Werror` reports warnings as errors ([configuration](config.md)).

| Id | Rule | Default severity |
|---|---|---|
| `LINT-001` | `unused-signal` | warning |
| `LINT-002` | `width-mismatch` | warning for truncation, info for extension |
| `LINT-003` | `incomplete-case` | warning |
| `LINT-004` | `combinational-loop` | error |
| `LINT-005` | `multi-driver` | warning |
| `LINT-006` | `casez-misuse` | warning |
| `LINT-007` | `if-latch` | warning |
| `LINT-008` | `naming-style` | info |
| `LINT-009` | `blocking-in-always_ff` | warning |
| `LINT-010` | `nba-in-always_comb` | warning |
| `LINT-011` | `unused-parameter` | info |
| `LINT-012` | `implicit-net` | warning |
| `LINT-013` | `case-default-missing` | warning |
| `LINT-014` | `comparison-width-mismatch` | warning |
| `LINT-015` | `unconnected-port` | warning |
| `LINT-016` | `mixed-assignments` | error |
| `LINT-017` | `undriven-signal` | warning |
| `LINT-018` | `incomplete-sensitivity-list` | warning |
| `LINT-019` | `out-of-range-select` | warning |
| `LINT-020` | `xz-logical-equality` | warning |
| `LINT-021` | `duplicate-case-item` | warning |
| `LINT-022` | `empty-implicit-sensitivity` | warning |
| `LINT-023` | `assignment-in-condition` | warning |
| `LINT-024` | `casex-statement` | warning |
| `LINT-025` | `wide-index` | warning |

| Id | Severity | Meaning | Disable |
|---|---|---|---|
| `LINT-000` | info | lint counts; printed when lint reported a warning or worse, and always with `--lint-only` | - |

## CODEGEN

Lowering failures are grouped into families by their text until each site gets its own id; a failure carries a location where the lowering had the source node at hand.

| Id | Severity | Meaning | Disable |
|---|---|---|---|
| `CODEGEN-000` | info | end-of-phase counts, printed when the phase reported a warning or worse | - |
| `CODEGEN-001` | error | a construct or form llg does not simulate yet | - |
| `CODEGEN-002` | error | an illegal design found during lowering (conflicting drivers, illegal targets, process rules) | - |
| `CODEGEN-003` | error | an operand or target of the wrong type or width | - |
| `CODEGEN-004` | error | a system task or method called with the wrong number or kind of arguments | - |
| `CODEGEN-005` | error | a design exceeds a runtime or generator limit | - |
| `CODEGEN-006` | error | an internal invariant of the generator failed (a bug in llg) | - |
| `CODEGEN-009` | error | any other lowering failure | - |
| `CODEGEN-010` | warning | a lowering warning (ignored annotation, approximated construct) | - |

## OPT

No messages.

## BUILD

| Id | Severity | Meaning | Disable |
|---|---|---|---|
| `BUILD-001` | error | a filesystem operation of the model build failed | - |
| `BUILD-002` | error | a `--cflags` token cannot pass through the CMake cache | - |
| `BUILD-003` | error | the generated model has an incompatible value ABI | - |
| `BUILD-004` | error | an invalid value backend selection or missing GMP input | - |
| `BUILD-005` | error | a `--dpi-lib` library is missing or unusable | - |
| `BUILD-006` | error | CMake could not be started | - |
| `BUILD-007` | error | CMake configuration failed; the CMake output follows unchanged | - |
| `BUILD-008` | error | CMake found no build program for the generator | - |
| `BUILD-009` | error | the C compiler failed; its output follows unchanged | - |
| `BUILD-010` | error | the model link left DPI-C symbols undefined | - |
| `BUILD-011` | error | an import library was requested from a non-MSVC toolchain | - |
| `BUILD-012` | error | the build produced no model executable | - |
| `BUILD-013` | error | the build produced no runtime archive | - |
| `BUILD-014` | error | another process did not finish populating the runtime cache | - |

## RUN

| Id | Severity | Meaning | Disable |
|---|---|---|---|
| `RUN-001` | error | the model executable could not be started | - |
| `RUN-002` | error | the model was terminated by a signal (exit 1) | - |
| `RUN-003` | error | the model exited with a status outside 0-255 (exit 1) | - |
| `RUN-004` | error | the model ended without an exit status (exit 1) | - |

## SIM

| Id | Severity | Meaning | Disable |
|---|---|---|---|
| `SIM-000` | info | end-of-run counts, printed at every exit when the run reported a warning or worse | - |
| `SIM-001` | info | $finish ran (finish number 1 or 2) | $finish(0) |
| `SIM-002` | info | $stop ran (finish number 1 or 2) | $stop(0) |
| `SIM-003` | info | $info; the user text follows the frame unchanged | - |
| `SIM-004` | warning | $warning; the user text follows the frame unchanged | - |
| `SIM-005` | error | $error; the user text follows the frame unchanged | - |
| `SIM-006` | fatal | $fatal; the run ends with exit status 1 | - |
| `SIM-007` | error | an assert, assume or expect failed without an else action | an else action block, $assertoff, $assertfailoff |
| `SIM-008` | info | run statistics printed by $finish(2) and $stop(2) | a finish number below 2 |
| `SIM-009` | error | a run-time error the simulator detects in the design (bad cast, out-of-bounds access that stops the run) | - |
| `SIM-010` | error | a run-time error of a runtime service (scheduler, synchronization, file I/O, coverage, constraints) | - |
| `SIM-011` | warning | a run-time warning of a runtime service | - |
| `SIM-012` | warning | a dynamic array, queue or associative array operation fell back to the default (IEEE 1800-2009 7.4-7.10) | - |
| `SIM-013` | warning | $readmem/$writemem file warning | - |
| `SIM-014` | error | $readmem/$writemem file error | - |
| `SIM-015` | warning | a timing check violation or SDF annotation message | --no-timing-check-messages |
| `SIM-016` | error | a VPI application or plugin error | - |
| `SIM-017` | error | a DPI-C bridge error | - |
| `SIM-018` | error | the waveform writer failed | - |
| `SIM-019` | error | the run ended without $finish: deadlock, zero-delay loop or step limit | - |
| `SIM-020` | warning | a model environment setting (LLG_SIM_LOG_FILE, LLG_STOP_POLICY, plusargs) is unusable | - |
| `SIM-021` | info | runtime information (randomize statistics, stack limit notes) | - |
| `SIM-030` | fatal | an internal runtime invariant failed; the model aborts | - |
| `SIM-031` | fatal | the model could not allocate memory; it aborts | - |

