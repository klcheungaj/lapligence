# Emission: model lifecycle, sharing and memory

## Model lifecycle and checks

Initialization frames allow legal zero-time declaration calls, not timing-bearing
functions. Tear down runtime queues and VPI observers before model storage.
`llg_model_start`/advance/close preserve suspended state; advance returns status 2
without cleaning up that live context. Reject double start, allow repeated
start/close, and preserve `LLG_MODEL_NO_MAIN` for host-controlled entry. The process-global runtime
supports one model, not concurrent/thread-safe instances. Keep selected value ABI
4 (legacy) / 5 (compact) and cache markers aligned; reject stale generated C.
Frame estimates use the selected descriptor: LP64 `sv4_t` is 32 / 24 bytes and
`llg_vpi_arg_t` retains conservative bounds of 64 / 56 bytes (actual 56 / 48).
Emit descriptor and complete-frame size guards
on 64-bit hosts, each distinct `_Static_assert` once per model (bodies that
share a frame type share its guard). Shared root-frame guards include the added record pointer and
its alignment; the callee arena-selection estimate excludes that root-only field. Emit `LLG_MODEL_PROCESS_ABI 3`, pass
only immutable descriptors to spawn/fork sites, and initialize through
`llg_rt_init_with_args_and_precision`; coroutine stack sizing is not model data.
Default and teardown calls for plain static storage use one file-scope pointer
table and loop per value representation, flushed at the storage section's
original position. Nets are declared zeroed (`static llg_net_t g_net_0, ...;`)
and described by data: interned per-net shapes (`llg_net_shapes[]`: width,
sign, resolution, fill, delays, driver count, interned strength, segment and
driven-run tables) and one `llg_net_storage[] = { net, shape }` row per net.
Each start runs one `llg_net_layout` loop that configures every net from its
shape and takes its contribution cells, driver pointers, index nodes, overlap
scratch, path-bit and segment records from the front of exact-size pools
(`llg_net_cell_pool`, `llg_net_driver_pool`, ...) in net order, so the driver
pointers address cells as before. Optional strength views stay per net
(`g_net_<k>.strength = &...;`, NULL unless a `%v` reads the net) and
`IrDisplayArg::Strength` formats with `LLG_FMT_STRENGTH`. Net defaults, driver
reset/destruction, index reset, driven-run starts and alias-list cleanup are
fixed-size loops over `llg_net_storage`, independent of net count, driver
count, width and resolution kind; only mutable index state needs resetting. Scalar aliases copy all
visible owners before binding; array aliases copy and bind in original array/
element order through one pointer table. Array initialization refreshes only its
contiguous slice of that table. Runtime queues and VPI must close before these
loops destroy values and clear alias lists; repeated start/close rebuilds them.
Runs of consecutive constant startup calls (spawns, finals,
waveform registrations) use argument tables and loops in their original order;
registrations still fail startup. Generated identifiers are standard C (never
`$`, checked by the all-fixture lint).

`names.rs` owns source-name escapes (encoding: `ident` docs). Lowering retains raw source components separately from
display paths: never pass encoded components through `ident` again. Keep the
source diagnostic labels, VPI and waveform metadata, design/export
file names and foreign DPI C symbols independent of internal symbol spelling.
After frame sharing and all derived symbols are emitted, cap complete internal
identifiers at `MAX_C_IDENTIFIER_LEN` (32) through a sorted per-model registry.
Sort only the identifiers needing replacement; use hash sets for occupied-name
lookup and borrow substitutions rather than allocating per occurrence.
A replacement keeps its namespace, as many leading original-name characters
as fit, a `_h` plus base-36 registry index and any `_frame_t`/`_desc`
suffix. Leading stems retain hierarchy context without decoding source escapes;
the index makes stems with the same leading characters distinct. Reserve all
occupied identifiers, previously assigned replacements, explicit foreign DPI
names and identifiers from the embedded runtime headers, including header names
absent from the model. Runtime API/macro names and foreign DPI names are exempt
from the internal bound. The existing emitted-C identifier rewrite passes
skip literals/comments/numeric suffixes; this does not admit opaque IR fragments.
`GeneratedModel::sources()` includes `model.c` and `model.symbols.tsv`. The map
contains every renamed symbol as `short<TAB>original<NEWLINE>`, sorted by short
name, with no header; it is empty when no renames occur. The CLI writes it for
normal builds and `--gen-only` exports. Pass both files as builder extras: CMake
compiles only `.c` files and the current extra-file set owns stale-output cleanup.
Regeneration replaces the map, including with an empty map; a source set omitting
it removes the old sidecar. Keep separate-process determinism tests on both files.
Coroutine descriptor
labels and loop budgets use source process/function labels; branch descriptors use
the source owner plus `.fork`, never the helper C symbol. A shared root class's
one descriptor has a NULL name: the runtime then names the running process in
HDL backtraces, and the spawn table supplies each instance's label. IR functions without source
provenance use an unnamed-function diagnostic label.

Maintain exact scope/reference checks, array-index disposal, inert invalid handles
and source-size preflight regressions. Preserve these distinct validation layers:
Rust structure checks, real IR-to-C emitter execution, public HDL CLI runs and
handwritten C components. Hand-authored output-shape probes do not execute this
emitter. Use [ownership validation](../../../../tests/readme.md#dynamic-ownership-validation)
and its generated-C sanitizer configuration; do not infer feature acceptance from
component tests.

Frame-name fallback to model storage: `owned/storage_names.rs` module docs. Do
not add per-access scans of model signals, functions or objects: rendering must
stay linear in instances.

## Instance body sharing

`model/sharing.rs` groups compiler-owned emitted bodies by source location,
coroutine/ordinary ABI, exact canonical frame layout and typed operand shapes,
then requires exact equality after normalization. Unrepresented
references remain in the key. Widths, resume numbers, array extents and ordinary
parameter payloads remain structural; ULL payloads and selected wait-dependency
`.lsb` values of generate-instance bodies and port links (`IrProcess::port_link`,
whose selections are the instance's connection) may become record fields, and
in generate-instance bodies so may the constant bounds of `sv4_part_select` and
`sv4_part_select_to` (a genvar-dependent part-select actual folded per
iteration), as `int64_t` operands. Module
path sets and sequential UDP state records are pointer operands, so path-stamp
processes and UDP gates of one definition share a body. A body belongs to a
generate iteration when its label or diagnostic name spells an indexed scope
(`tb.g[3].always`: labels keep the written name, only C identifiers encode
it); the same test applies to coroutine processes, functions and branches and
to plain functions. Path and timing condition evaluators come from specify
blocks, which are never in a generate scope, so their payloads stay
structural; they share a body with the instance record passed as the set's
condition context. Net driver slots (`.drivers[slot]`, the slot argument of
`llg_net_write*`, `llg_nba_net_*` and `llg_inertial_*net`) and an SDF-annotated
gate's `llg_sdf_gate_delays` index are identity operands of every body.
A coroutine process's per-site mutable state (inertial handles, sparse-array
handle keys, delayed batch handles) is a file-scope object named after the
process (`Frame::site_static`), registered as a pointer operand, so instances
keep distinct state while sharing the body; other bodies keep function-local
statics. Event-expression evaluators (`IrPreFn::MonEval`/`RealEval`) stay one
per process: their `eval_context` is an owned frame reference, so a process
whose wait names one keeps its own body.
Only known activation/net-contribution identities become scalar operands.
Bodies with local static storage stay separate. Startup-external roots (assertion
actions) stay separate because they do not receive the startup record initialization.

Candidates enter exact sharing groups as they are rendered; each group retains
one normalized source and key. Every member retains its typed operands, including
each generated constant occurrence and its width/sign, even after its duplicate
normalized source is released. An operand is a shared kind (declaration, shape,
access) plus a value; registry entries and scalar literals are interned per pass,
never copied per occurrence. A member whose body the shared entry replaces drops
its rendered source.

## Generation memory

Rendering must not accumulate on top of the execution IR. Retained rendered
bodies are shrunk to their length; a finished `FrameLayout` drops its name and
access indexes (`release_emission_state`), keeping only the block tree, fields
and call slots that frame grouping, sharing and metadata read. The production
path (`render_with_value_config`) owns the `ExecutionModel`: it releases each
process's operations after that coroutine body is rendered, so a fact needed
later must be recorded on the artifact, and a changed
arena-callee set is reanalyzed in place. The borrowed test API keeps the model
intact and clones only for reanalysis. Assembly moves bodies into the model text
and drops artifacts before the identifier pass, which bounds identifiers in the
text's own buffer. Any change must keep `model.c` and `model.symbols.tsv`
byte-identical between the owned and borrowed paths.

`LLG_SHARE_MIN_INSTANCES` is a positive integer (default 4); `unlimited` disables
sharing. The default leaves pairs and triples independent, reserving record and
hidden-call overhead for classes with a larger reduction in repeated code. Each qualifying class emits one body and one static const record array
(`llg_body_instances_<k>[member]`). When every member's site table is equal a
root class also gets one descriptor and site table; its members are spawned by
`llg_model_spawn_class(class, first, count)` over `llg_model_body_classes[]`
(descriptor, records, stride, record and name offsets, an optional label table,
region), in their original spawn order.
Operands equal across all members stay direct constants/references. Shared root
frames add `_llg_instance` immediately after the header; spawn tables supply its
`offsetof` and initialize it before execution, including Reactive program roots.
Runtime labels and all model storage remain distinct; descriptors and site tables
remain distinct unless the class descriptor above replaces them.
Metadata offsets use the resulting frame type. Shared callees/fork entries keep
small ABI-preserving thunks forwarding a hidden record argument; their frame
layout and arena/poll analysis therefore do not change. Shared bodies resist
compiler inlining/cloning to avoid recreating the duplicated code.

The immutable record pointer is reconstructed at every entry, before dispatch;
it is not a C9 local whose initial assignment can be skipped on resume. Records
hold model-lifetime storage/descriptor addresses, never activation locals. The
frame lint must recognize shared-body signatures and narrowly permit addresses of
record-referenced model storage while retaining its cached-field and overlay checks.
