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
