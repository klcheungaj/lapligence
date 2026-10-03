# Known issues

Open design limitations that are understood but deliberately deferred. Each
entry states the symptom, the cause, the intended direction and how to
reproduce it. Remove an entry when the fix lands.

## Unpacked net arrays are expanded one net per bit

**Status:** open; to be addressed with the type-system refactor.

### Symptom

An unpacked array of multi-driven nets, such as `wire [W-1:0] r[N]` with two
continuous drivers, a partial driver or a port connection, produces generated
C whose size, compile time and simulation work grow with `N × W`, even when
every bit of an element is connected identically.

Example: [`tests/fixtures/sim/continuation_20_23/continuous_contexts.sv`](../tests/fixtures/sim/continuation_20_23/continuous_contexts.sv)
is 88 lines, with four `continuous_case` instances at `W` = 1, 7, 65 and 129.
It generates a `model.c` of about 7.1 MB and 96,000 lines:

- 3,838 separate width-1 `llg_net_t` objects, each with its own driver,
  strength, index and scratch tables (387 per three-element array at
  `W = 129`);
- `llg_model_storage_defaults()` and `llg_model_storage_destroy()` of about
  16,500 lines each, one reset/destroy sequence per bit net;
- continuous-assignment processes of up to about 3,600 lines, one unrolled
  block per bit (clone, bit select, cast, `llg_net_write` to that bit's net).

The test that runs it (`sim_review_tasks20_23`
`continuous_arrays_keep_values_dependencies_and_static_pattern_topology`) is one
of the slowest in the suite because it compiles this model in both optimizer
modes.

### Cause

A net-array cell has one electrical group per bit
([`collection/net_arrays.rs`](../src/sim/codegen/lowering/collection/net_arrays.rs)
creates the `g_array_net_<array>_<element>_<bit>` groups;
[`collection/processes.rs`](../src/sim/codegen/lowering/collection/processes.rs)
contributes each bit through its own driver slot). Per-bit groups are the
simplest representation that is always correct: nets resolve per bit, and port
collapse, `alias`, partial drivers such as `window[3:2] = a[-1:0]`, and
per-bit force/release can join bits of different nets. The representation is
used unconditionally, however, so the common case of identically connected
bits pays the per-bit cost in storage, code and run time (one driver scan per
bit instead of word-parallel `sv4` resolution).

### Intended direction

1. **Range-partitioned electrical groups.** Split a net only at the boundaries
   where its connectivity changes (driver ranges, alias and port-collapse
   points, force targets, net-type changes) and give each maximal run of bits
   with identical connectivity one group of that width. Identically connected
   elements then resolve as one wide group each; the per-bit form remains the
   degenerate case, so semantics are unchanged.
2. **Loops over remaining bit groups.** Where bit-level groups are genuinely
   needed, emit a loop over a table of groups instead of one unrolled block per
   bit.
3. **Table-driven net storage (done).** Driver cells use one array per net;
   defaults, index resets, alias binding/cleanup and destruction use descriptor
   tables and loops. Items 1 and 2 remain open.

The net, strength, alias, port-collapse and force/release suites are the
oracles; a differential fixture across widths and connection patterns should
accompany the change.

### Reproduce

```sh
llg --gen-only --top tb --edition 2009 --out-dir <dir> \
    tests/fixtures/sim/continuation_20_23/continuous_contexts.sv
grep -c '^static llg_net_t g_array_net_' <dir>/sim/*/model.c
```

## Wide-value concatenation and part-selects copy one bit at a time

**Status:** open; deferred.

### Symptom

Simulation time of designs that concatenate or part-select wide packed values
grows with the number of bits copied, not with the number of 64-bit words. In
the `wide-values-default` configuration of
[`perf/scripts/corpus.sh`](../perf/scripts/corpus.sh) (128 tasks on
4,096-bit values), `sv4_lsb_bit` and `sv4_lsb_bit_set` take about 79% of the
simulation's CPU samples.

### Cause

[`sv4_concat`](../src/sim/rt/value/operations.c) and the packed selection-plan
read, slice and write paths in
[`value/selection_plan.c`](../src/sim/rt/value/selection_plan.c) copy each bit
with `sv4_lsb_bit` and `sv4_lsb_bit_set`. That is two calls and a limb lookup
per bit for value, X and Z planes, where a contiguous range could move up to
64 bits per step.

### Intended direction

Copy contiguous ranges with word-level shift-and-mask operations over the
value, X and Z limbs. Keep a bit loop only for reversed slices. The
four-state value suites and the `wide-values` corpus stdout hashes are the
oracles; add a differential test across widths, offsets and limb boundaries.

### Reproduce

```sh
perf/scripts/corpus.sh --sim-bin <llg> --size standard --mode default --runs 1 \
    --scratch-dir <scratch> --output-dir <out>
```

Profile `wide-values-default` with the sampler described in
[`perf/README.md`](../perf/README.md).

## High frontend memory use during Slang wrapper capture and import

**Status:** open; memory reduction deferred.

### Symptom

Large elaborated designs require substantially more frontend process memory
than the size of their exported semantic data. In Linux x86-64 release
measurements of `many_processes_registers_config` with two clock edges:

| Register processes | Exported records and strings | Generation peak RSS |
| --- | --- | --- |
| 20,000 | 320,406,270 B (about 306 MiB) | about 3.28 GiB |
| 40,000 | 640,766,270 B (about 611 MiB) | about 6.54 GiB |

These are whole-process peaks across compilation, capture, owned import and
C generation. They do not isolate the C++ wrapper's share and are not
generated-simulator runtime memory measurements. Export size is approximately
linear for this corpus; other design shapes can have different costs.

### Cause

The [C++ wrapper](../src/wrapper/slang_c_api.cpp) materializes a complete
`LlgSlangSnapshot` while Slang's compilation is still live. Capture also uses
identity maps, pending-edge tables and separately stored strings. For this
corpus, each small register process contributes roughly 60 semantic nodes,
90 edges and 12 constants: about 16 KB of charged export data. Charged bytes
exclude container capacity, indexing overhead and Slang's own allocations.

The [safe FFI decoder](../src/ffi/slang.rs) copies native snapshot data into
owned Rust data before destroying the native owner. The
[driver](../src/bin/llg.rs) then imports the Rust snapshot into the owned DB
and builds semantic/execution IR. These stages have overlapping
representations; the native compilation, native snapshot and later Rust IR
are not all retained together. A stage-by-stage allocation profile is still
needed to quantify each contributor.

`export byte limit exceeded` originates in llg's wrapper capture budget.
Raising that budget admits larger exports but does not reduce their memory
cost or bound total process RSS. A large design can still exhaust its export
budget, a record-count ceiling or available process memory.

### Intended direction

Profile peak allocations by stage, then reduce retained copies and export
overhead: intern repeated constant/string payloads, evaluate compact records
or simulator-specific capture, and investigate chunked capture/import.
Preserve checked C ABI ownership and the single owned DB import; consumers
must not traverse native ASTs independently. Verify exact values, source
identity and diagnostics as well as generated-model behavior.

Use `--max-export-mib` to choose the simulator export budget and
`LLG_MEMORY_LIMIT_MB` for the optional process-wide memory guard. Budget for
the measured frontend peak, rather than the exported byte count.

### Reproduce

After building the export-size fix, run:

```sh
/usr/bin/time -v llg --gen-only --max-export-mib 4096 \
    --top many_processes_registers_config \
    --define LLG_CORPUS_N=40000 --define LLG_CORPUS_EDGES=2 \
    --out-dir <dir> perf/corpus/many_processes.sv
```

Read maximum resident set size from `time`; lower the process count to 20,000
for the smaller comparison. The export budget counts captured data, not the
bytes of generated `model.c`.
