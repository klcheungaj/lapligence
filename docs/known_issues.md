# Known issues

Open design limitations that are understood but deliberately deferred. Each
entry states the symptom, the cause, the intended direction and how to
reproduce it. Remove an entry when the fix lands.

## Declared net views retain per-bit binding metadata

**Status:** electrical groups and continuous writes are range-partitioned;
compact declared-view metadata remains open.

### Symptom

Wide unpacked net arrays still emit one alias-view binding row per declared bit,
so descriptor data grows with `N × W`. Identically connected bits now share wide
electrical groups and word-parallel resolution; contribution writes use contiguous
part selects and descriptor loops for remaining groups.

Example: [`continuous_contexts.sv`](../tests/fixtures/sim/continuation_20_23/continuous_contexts.sv)
is 88 lines, with four `continuous_case` instances at `W` = 1, 7, 65 and 129.
An earlier snapshot before table-driven storage and operand-traffic elision
emitted about 7.1 MB and 96,000 lines. A separate Linux release comparison at
base `c260a74c` and after range partitioning measured:

| Metric | Before | After |
| --- | ---: | ---: |
| `model.c` bytes | 5,638,045 | 1,008,174 |
| `model.c` lines | 67,415 | 12,259 |
| Array electrical groups | 3,838 | 76 |
| Generation seconds | 0.366 | 0.071 |
| Clean model build seconds | 18.089 | 3.063 |
| Build and execution seconds | 49.994 | 3.967 |

Current emission combines table-driven storage lifecycle work, eligible operand
borrows and matching-cast elision with range-partitioned groups and contribution
loops. Per-row contribution casts remain explicit because width/sign metadata
can differ between rows; declared-view binding metadata still grows with `N × W`.
The `sim_review_tasks20_23` test
`continuous_arrays_keep_values_dependencies_and_static_pattern_topology` compiles
this fixture in both optimizer modes.

Timings above are indicative, with eight build threads and a shared runtime cache.
The 64-element, 128-bit toggling-driver
[`runtime.sv`](../tests/fixtures/sim/net_partition/runtime.sv) witness emits 64
array groups instead of 8,192; its model shrinks from 9,188,411 to 807,091 bytes.

### Cause

[`collection/net_partition.rs`](../src/sim/codegen/lowering/collection/net_partition.rs)
combines adjacent canonical roots while member mappings, structural driver ranges,
force/release targets and effective type/delay owners agree. Declared net views
still use per-bit `IrNetAliasBinding` / runtime alias-part descriptors so waveform,
VPI and array dependencies retain their original shapes. Permuted mappings and
opaque projections conservatively retain bit groups. Short or disconnected
contribution gathers retain ordinary emission; homogeneous captured part-select
writes, including delayed writes, use descriptor loops.

### Intended direction

Represent declared alias views with range descriptors as well, preserving exact
bit correspondence, source names and observation/dependency behavior. This is
separate from electrical partitioning and table-driven net storage.

### Reproduce

```sh
llg --gen-only --top tb --edition 2009 --out-dir <dir> \
    tests/fixtures/sim/continuation_20_23/continuous_contexts.sv
wc -lc <dir>/sim/*/model.c
```

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

Run:

```sh
/usr/bin/time -v llg --gen-only --max-export-mib 4096 \
    --top many_processes_registers_config \
    --define LLG_CORPUS_N=40000 --define LLG_CORPUS_EDGES=2 \
    --out-dir <dir> perf/corpus/many_processes.sv
```

Read maximum resident set size from `time`; lower the process count to 20,000
for the smaller comparison. The export budget counts captured data, not the
bytes of generated `model.c`.

## Frontend and C generation time grow superlinearly with design size

**Status:** open; deferred.

### Symptom

Doubling the number of processes more than doubles `llg --gen-only` time. In
Linux x86-64 release measurements of `many_processes_registers_config` with
two clock edges (shared host, medians):

| Register processes | 5,000 | 10,000 | 20,000 | 40,000 |
| --- | --- | --- | --- | --- |
| Total generation | 5.7 s | 11.7 s | 26.4 s | 69.4 s |
| Frontend compile and copy | 0.97 s | 2.01 s | 5.66 s | 19.16 s |
| Lowering and C generation | 3.30 s | 6.38 s | 13.41 s | 29.18 s |
| of which C rendering | 1.53 s | 3.12 s | 6.80 s | 14.79 s |

Per doubling, total time grows 2.0×, 2.3× and 2.6×. The frontend is the
fastest-growing stage (3.4× from 20k to 40k).

### Cause

Not yet isolated. The repeated full scans previously found in lowering
(alias bit expansion, signal-global and structural-driver lookups, spawn
labels) and in the wrapper's edge lookups have been removed, and the lowering
combinational pass is now linear. The remaining growth sits in Slang
compilation/elaboration, wrapper capture and the owned copy, and in C
rendering; it may also include allocator and cache effects at these heap
sizes (see the frontend memory entry above).

### Intended direction

Profile each stage at 20k and 40k with release binaries, separating Slang
elaboration from wrapper capture and the Rust copy, and look for remaining
per-item work that scales with design size. Generated `model.c` must stay
byte-identical; the export-size scaling tests in
`src/sim/codegen/lowering/collection/nets/tests.rs` are the pattern for
work-count regression tests.

### Reproduce

Use the command in the frontend memory entry with `LLG_CORPUS_N` set to
5,000, 10,000, 20,000 and 40,000, and compare wall times.

## Record member net aliases rejected by the frontend

**Status:** open; alias grammar and frontend admission are owned by RTL-011.

Slang rejects a member of a fixed record net as a non-net in an `alias` statement,
although ordinary member continuous drivers and inout record connections work.
For example, declare `typedef struct { logic [7:0] lane; } record_t;`,
`wire record_t values[1:0];` and `wire [3:0] mirror;`, then use
`alias values[0].lane[3:0] = mirror;`. Compilation reports that `lane` is not a
net before owned capture. Audit IEEE 1800-2009 §10.11 and the net-lvalue grammar
in §A.8.5, then apply a narrow tracked frontend patch for the legal forms; do
not bypass checked compilation or treat frontend rejection as an illegal-type
oracle. This admission gap prevents member alias identity tests from reaching
the otherwise shared electrical projection path.


## Runtime-selected module reference connections have no qualified binding oracle

The fixed module-ref path requires constant actual selectors. A connection such
as `child c(values[index]);` rejects with an explicit runtime-reference-rewiring
diagnostic; it must not silently reread `index` on each access or invent a
canonical storage cell. IEEE 1800-2009 §23.3.3.2 describes hierarchical reference
binding, but the retained runtime-selector characterization has no adjudicated
binding/rebinding oracle. Qualify that boundary before enabling runtime-selected
connections. Static selected connections and nested packed projections execute.
