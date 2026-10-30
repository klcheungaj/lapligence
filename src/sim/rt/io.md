# Runtime: I/O, VPI and waveform

Keep hosted C descriptor namespaces separate: FD bit 31 with preopened standard
streams, MCD bit 0 stdout and bits 1..30 channels. Fan-out applies only to MCDs.
Typed scanners validate
suppressed assignments and leave unmatched delimiters unread; targets are borrowed
for calls. `$system` requires explicit `LLG_ALLOW_SYSTEM`; retain host C status and
distinguish omitted system(NULL) from an empty command.

`scheduler/output_files.c` reads run-time output settings at every init:
`LLG_SIM_OUT_DIR` (created if missing) prefixes relative write paths only —
waveform, `$fopen` w/a modes, `$writemem*`, the log — never reads;
`LLG_SIM_WAVE_FILE` replaces `$dumpfile`/`dump.vcd`; `LLG_SIM_LOG_FILE` starts a
once-per-process forked tee (POSIX only; Windows reports an error) that survives
simulator crashes, shares one pipe when stdout/stderr reach the same file, and is
joined at exit. `llg_wave.c` reads `LLG_SIM_OUT_DIR`/`LLG_SIM_WAVE_FILE` itself in
`llg_wave_model_init`, keeping the waveform unit scheduler-independent.
`llg_wave_start` (generated for `llg --wave`) records its file as the override
unless `LLG_SIM_WAVE_FILE` set one, opens it and selects at time zero after all
registrations; afterwards `$dumpfile`/`$dumpvars` are ignored while the other
dump controls still apply.

Memory images preserve X/Z, comments, addresses, views and loaded prefixes on
failure. Keep edition-specific default address order, explicit direction, short-word
padding, truncation/original-width enum validation and sparse-file diagnostics.
For the admitted signed `@` extension, check hexadecimal magnitude against the
signed index range before applying the sign; leading zeroes cannot affect the
index, and selected-view bounds must be checked before writes.
`memory_io.c` reaches fixed arrays, packed images (`llg_image_memory_*_view`
on an `llg_memory_image_t`, cell 0 in the most significant bits; a published
image is a variable's own storage, loaded in place under the `llg_ba` force
and region rules and published once like `sig_write_ranges`) and dynamic arrays or queues
(`llg_dyn_memory_*`/`llg_queue_memory_*`, current size, never resized; a
`total` 0 slice must lie within that size, and reversed bounds select no
element; one contents notification after a load), and dynamic arrays or
queues of fixed arrays (`llg_*_rows_memory_*`: one `llg_value_t` element;
cell `i` is item `i` of that row) through one storage abstraction, so parsing, addressing and range checks
stay shared. Integral-index associative arrays (`llg_assoc_memory_*`) share
the tokenizer and word conversion but address by key: start, finish and wide
`@` addresses are cast to the index type, each word is an
`llg_assoc_set_integral`, and a dump writes `@key` before every entry in index
order (S27-D15..D17). Associative arrays of fixed arrays
(`llg_assoc_rows_memory_*`, an `llg_assoc_value_t`) take one element (`row`
is the key; a load creates a missing element after the file and range are
validated through `llg_memory_storage_resolve`, a dump of a missing or
unknown key writes the default value, S27-D23..D25); the whole container is
not a memory (S27-D22). Report through
`llg_memory_warning` (missing file, word-count mismatch, X/Z conversion, empty
resizable array) or `llg_memory_error` (malformed token, address outside the
task range or memory, enum value, invalid range, a generated `origin` of
`UINT64_MAX` marking a runtime index or slice position outside the memory,
I/O failure); their effect and the `$writemem*` range check are in `memory_io.c` comments
(decisions S27-D5..D9 in `docs/lrm_decisions.md`).
`$sreadmemb/$sreadmemh` (ADV-026) call the same read entry points inside a
`llg_memory_text_begin(data, length)` / `llg_memory_text_end()` pair the
emitter generates: `llg_memory_open_read` then serves the text instead of
opening the file `path` names (which only labels the diagnostics, printed as
`memory string`), through the `llg_memory_stream_t` that every parser function
reads, so file and text loads share one tokenizer, address and range rules
(S26-D16..D20). The text is not copied and not NUL-terminated; the stream stops
at `length`. One slot suffices because a load never suspends. Do not add a file
read path that bypasses `llg_memory_open_read`.
Binary rank-one `$fread` advances low-to-high HDL addresses independently of physical
declaration order, clamps count at the high bound, and consumes/writes nothing for
invalid/unknown bounds or negative count. Keep element owners across callbacks;
this does not qualify multidimensional binary reads.
Scans (`scanning.c`) assign a destination only after its conversion
succeeds. `%u`/`%z` read 32-bit little-endian vecval words (aval then bval; this assumes
a little-endian host, true of every supported target),
assign nothing on short data, and `$fwrite` writes the same encoding. `%t`
scales in decimal through the `$timeformat` unit into the caller's unit.
Integral `$sscanf` sources drop leading zero bytes; NUL is white space.
Container `$fread` notifies once after the read, and a short final word fills
only its high bytes. `$ungetc` returns 0, and a successful seek clears the
slot's error and EOF state. Invalid descriptors leave destinations untouched; `$fread` then ends the run and
the other input functions return their failure code (`file_io.c`,
`lrm_decisions` S26-D*).

Monitor registration/enabling forces a postponed report; only registered signal
changes dirty later reports. `$monitor` owns one list (a new call replaces it)
governed by the `$monitoron/$monitoroff` flag, which outlives the list; any
number of `$fmonitor` lists are independent entries that `$fclose` removes with
their last channel. An invalid descriptor registers nothing. Strobes wait for
settled NBA-triggered work and print before the slot's monitors. `%t` retains caller units; design-wide timeformat
never changes scheduler time. Integer queries use quotient/remainder, halves up;
realtime is fractional. The runtime consumes integer design-precision ticks;
codegen owns scaling.

The VPI object model the runtime serves (every object type with its
properties, relations and iterations, and the status of each) is recorded in
[vpi_catalog.txt](vpi_catalog.txt), which `tests/runtime_vpi_catalog.rs`
keeps in step with the dispatch of `llg_vpi.c`, `llg_vpi_dynamic.c` and
`llg_vpi_runtime.c` and the
ADV-019 catalog walk checks against a run. A dispatch case added for a
property, relation or iteration needs its catalog row; a query outside the
catalog fails with an error (decision A19-D26). `tests/fixtures/runtime/vpi_annex_kml.tsv`
holds the Annex K, L and M listings the headers are checked against.

VPI handle encoding and lazy catalog indexing: `llg_vpi.c` header and catalog
comments; keep model init O(1). Slots made inside a compiletf/calltf (argument handles and
iterators, element/select arguments) are owned by that invocation and released
when it returns; the call handle itself persists per call site with its user
data (every request shares it; releasing it is a no-op, 37.2.2). sizetf runs once per registration after the first site's compiletf with no
active call. Retain error records, real catalog width 0 and simulator-owned value
buffers valid until the next query/shutdown. Puts go through `llg_ba`/`llg_ba_d`
(no delay), delayed calls (`llg_schedule_delayed_call`: delayed modes, one
carrier process per pending `vpiSchedEvent` record, written in its Active
turn, decision S42-D5) or the force table (`llg_force_value_parts`, `llg_force_real_value`, release through
`llg_release_parts`/`llg_release_real`). Tear down VPI/runtime queues before
model storage.
`vpi_mcd_*` (38.24-38.29) run on the `$fopen` table through `llg_file_mcd_*`
and `llg_file_name` (`llg_rt.h`): channels opened on either side work on both,
VPI writes are flushed per call like `$fdisplay` so the two interleave in call
order, closing a channel cancels its monitors and strobes as `$fclose` does,
and each slot keeps the name it was opened with. A file descriptor (bit 31) is
refused except by `vpi_mcd_name` (A20-D1..D4).
`vpi_get_value_array`/`vpi_put_value_array` (38.16, 38.35) work on whole
unpacked array rows: the section is validated (array, element type, format,
indices, count) before any value moves, gets fill one simulator buffer
(`array_values`, overwritten by the next call) unless `vpiUserAllocFlag`
names the application's, and puts write element by element through
`llg_ba`/`llg_ba_d` like `vpi_put_value` with `vpiNoDelay`. `vpiPropagateOff`
is accepted and every fanout is still notified (A20-D5..D8).
`cbForce`/`cbRelease` use `llg_force_watch_hook` and `cbDisable`
`llg_disable_watch_hook` (`llg_rt.h`): the runtime calls them after a force or
release statement, or a `disable` that ended something, behind one unlikely
branch. The force hook gets the statement's parts; `llg_vpi.c` maps them back
to catalog rows through a storage-address index built on the first such
registration (net array elements through the alias that publishes them) and
reports a `SLOT_STMT` handle released before the dispatch returns. VPI's own
force/release puts do not report (A20-D9). Rows of named blocks, tasks and
functions carry their `llg_disable_target` identity
(`LLG_VPI_OBJ_ACTIVATION`) for `cbDisable` (A20-D10).
Assertion callbacks (39.4) use `llg_assertion_watch_hook`. The runtime raises
an attempt event (start, success, vacuous success, failure, disabled, killed)
with the attempt's start time only for an assertion item whose `watches`
count `vpi_register_assertion_cb` raised; every engine attempt record
carries its start time for this. Events raised inside the assertion engines
are queued and delivered when the outermost engine call returns
(`assertion_events_enter`/`leave` in `coverage/assertions.c`), so a callback
may call `vpi_control`, `vpi_put_value` or remove callbacks without seeing a
half-updated attempt list. The Observed pass that evaluates clock ticks
leaves through `assertion_events_leave_tick`, which sorts its batch by
instance group, then latest start first, then queue order (A17-D9); an
unwatched model never queues, so it never sorts. The generated model calls
`llg_vpi_procedures_started` after spawning its time-zero procedures and
before the always_comb/always_latch ones; when an application registered a
system task or a callback, it queues the Active event that runs
`cbAssertionSysInitialized` and `cbAssertionSysOn` (A17-D10); without one
the run's scheduler passes are unchanged. `vpi_control` assertion operations go through
`llg_acov_control`: concurrent assertions change their runtime record,
immediate and deferred ones the item's `control` bits, which the generated
code of a model with a VPI catalog reads per check. The item table stays
bound after the runtime cleanup so `cbEndOfSimulation` reads final counters
(A17-D1..D3). FSM handles (`SLOT_FSM`, `SLOT_FSMSTATE`) index the FSM
coverage table of `coverage/fsm.c` (A17-D13); that table's visits stay
bound after cleanup too. FSM coverage shares the value-watch rows of
`cbValueChange` (`value_watch_notify`), so a removed callback leaves the
FSM's watch in place. An item's `code` names its entry in the expression object
table (models generated with `--vpi`): the entry is not a handle; its
children are the 37.46/37.51 relations (`vpiClockingEvent`,
`vpiDisableCondition`, `vpiProperty`, `vpiExpr`, `vpiStmt`, `vpiElseStmt`).
Sequence and property declarations are table roots scoped to their
instance; an instance links to its declaration through a
`LLG_VPI_CODE_LINK` entry. A record placed on an instance
(`instance_code`) watches the item whose whole property it is and maps a
vacuous success to a success (A17-D4, A17-D5).
Built-ins are never dispatched to a registration under their name: after
plugin startup, `llg_vpi_check_builtin_overrides` fails the start, naming the
call, when an application registered a built-in the model runs natively
(decision S99-D6). It returns at once without registrations; the generated
model emits the table and the call only when it calls built-ins.

VPI callbacks (SIM-043) are records owned by the registry from registration
until removal or, for time and action reasons, retirement after they run.
Fired copies (`SLOT_FIRED_CB`, S43-D4, `LLG_VPI_MAX_FIRED_CALLBACKS`): `llg_vpi.c`
comments.
Time reasons are one-shot region callbacks in their Table 4-1 PLI region
(`cbNextSimTime` through `llg_time_slot_hook` at each Pre-Active pass);
`cbReadWriteSynch` takes `LLG_REGION_PRE_NBA_PLI` and `cbNBASynch` the
following `LLG_REGION_PRE_NBA`, so work a read-write callback wakes runs
before `cbNBASynch` (S43-D1); a zero-delay `cbAfterDelay` registered after
Pre-Active goes to `LLG_REGION_INACTIVE` (S43-D2).
`cbValueChange` uses `llg_value_watch_add`: a waiter-less subscription on the
address's wait row that `sig_publish_changed`/`real_write` already look up,
whose hook runs after every runtime consumer; never add a separate per-store
lookup. A whole-array record holds one watch entry per element (bounded by
`LLG_VPI_MAX_ARRAY_WATCH_ELEMENTS`), so element writes stay on that path. Dispatch collects records in registration order before running any
(value changes then run newest first, S43-D6) and frees removed ones only when
the outermost dispatch ends, so callbacks may register/remove any callback.
Watches end with the runtime lifetime (`free_value_watches`), so VPI removal
after cleanup is a no-op.
A `cbValueChange` on a named event counts a watch on its event object
(`llg_event_watch_add`, `vpi_watches`): every trigger of a watched object calls
the same value-watch hook with the object as key after waking its waiters, so
the callback runs before the woken processes (A19-D8); unwatched objects pay one
predictable branch per trigger. The count belongs to the VPI layer, so
`llg_event_object_reset` keeps it.
Structure objects (ADV-019 family 2) are catalog rows without storage:
`conns` gives each its expression connections (port high connection, cont
assign lhs/rhs, primitive terminals) as compile-argument references plus
constant digits, which `conn_handle`/`read_conn` turn into the object, select,
element or a `SLOT_CONN` handle. Port bits, primitive terminals and interface
port reference objects are dynamic slots on their row (`SLOT_PORTBIT`,
`SLOT_PRIMTERM`, `SLOT_REFOBJ`). Unnamed rows (`LLG_VPI_OBJ_UNNAMED`) are left
out of the name index; instance array elements are found by composing the
array's full name with the indices.
Net and variable connectivity (vpiDriver, vpiLoad, their local forms,
vpiPortInst, vpiPort, vpiPrimTerm, vpiContAssign, vpiPathTerm, vpiTchkTerm,
vpiResolvedNetType; decisions A19-D33 to A19-D35) is derived at runtime: the
first such query builds a link table from the rows' `conns`, port targets
and the force statements of the code tables (one link per referenced bit
span, sorted by net), so a model whose applications never ask pays nothing
and lowering emits no extra tables. Forces are matched to the live entries
of the scheduler's force table (`llg_force_entry_view`) by their target
bits. VPI models never collapse ports, so the simulated net of 23.3.3.7 is
recomputed from the net-to-net port links. Continuous assignment bits are
`SLOT_CABIT` slots on the assignment's row.
Expression and statement objects (ADV-019 families 3 and 4) live in
`llg_vpi_code_tables_t` (registered by `llg_vpi_model_code`, NULL without a
catalog): one 48-byte `llg_vpi_code_t` per expression or statement, its
children contiguous after it with the relation that reaches each (`role`),
plus interned texts (names, literal digits), file names and select ranges.
Process, task, function and named-block rows, connections and call-site
arguments point at their root with `code` (index + 1). Entries flagged
`LLG_VPI_CODE_ALIAS` stand for a catalog row and hand out that row's
handle; `LLG_VPI_CODE_HIDDEN` entries are implicit conversions, skipped for
handles and applied when evaluating; `LLG_VPI_CODE_OPAQUE` entries answer
type, size and line only. Other entries become `SLOT_CODE` handles on
demand. `code_eval` evaluates a tree through the value facade with the
frontend's widths; `decompile` rebuilds the source spelling with Table 11-2
precedence into one buffer that each `vpiDecompile` overwrites.
Dynamic objects (VPI-O4, `llg_vpi_dynamic.c`) are catalog rows flagged
`LLG_VPI_OBJ_DYNAMIC` whose `storage` is the string, handle or container
global and whose `shape` indexes `llg_vpi_dynamic_tables`. Class objects,
properties and elements are dynamic slots (`SLOT_CLASSOBJ`, `SLOT_PROPERTY`,
`SLOT_DYNELEM`) that keep the object's address, never a reference the
collector sees; an element keeps (row or property, index or key) and
re-resolves at each access. Class definitions, typespecs and extends
objects are static slots after the catalog rows, followed by one
`SLOT_CLASSITEM` slot per `llg_vpi_class_item_t` (methods, their io
declarations, parameters); a class's items are contiguous, a method's io
declarations follow it and name it through `owner`. Field records carry the
rand qualifier, visibility, const flag and the index of the hidden
`rand_mode` off bit that `vpiIsRandomized` reads. A constraint item names
the hidden root code entry of its constraint items (37.30, 37.34) and its
`constraint_mode` off bit (an object field, or the static signal of a
static constraint); a constraint obtained from an object is a dynamic
`SLOT_CLASSITEM` slot that tracks the object like a property handle. A live table assigns `vpiObjId`s and marks
objects some handle names. The collector's reclaim hook marks the dying
objects, runs `cbReclaimObj`/`cbEndOfObject` (navigation and puts refused),
releases every slot and iterator into them and forgets their ids; with no
named object and no object callback it returns at once. A class variable's
`cbValueChange` watches the variable's `_llg_dep` change marker
(`LLG_VPI_WATCH_OPAQUE`, NULL value).
Runtime structures (`llg_vpi_runtime.c`, after `llg_vpi_dynamic.c` in the
same translation unit) answer before the shared dispatch: user systf and
callback lists from NULL, the time queue (`SLOT_TIMEQ`, one per distinct
future time from `llg_rt_pending_times`, which reads the scheduler queues
only when asked), UDP definitions from the code tables' `llg_vpi_udps`
(`SLOT_UDPDEFN`, `SLOT_UDPIO`, `SLOT_TABLEENTRY`), `vpiUse` from the
connectivity links and the code rows flagged as aliases, mapped to their
enclosing statement (A19-D43), and an iterator's `vpiUse` (its reference
object, recorded at `vpi_iterate`).
Threads and frames (A19-D44) are named, never pinned: a `SLOT_THREAD`
handle holds a process's registry slot and per-run identity, checked
against the scheduler at each use (`llg_rt_thread_live`); a `SLOT_FRAME`
handle adds the serial of one activation on the frame stack the bridge
keeps per process. A catalog model brackets each task and function body
with `llg_vpi_frame_enter`/`llg_vpi_frame_exit`, which run only while
`llg_vpi_frames_on` is set (an application's startup routines ran); a
model without an application pays one load and branch per call, measured
within noise on a call-heavy design, and tracking costs about 5% there.
The scheduler reports thread creation (roots with `LLG_DESC_PROCEDURAL`),
ends and watched turns through `llg_rt_set_thread_hook`, which the bridge
installs with frame tracking and removes at shutdown.
Module paths and timing checks (VPI-T1) are unnamed `vpiModPath`/`vpiTchk`
rows whose conns carry a role (`LLG_VPI_ROLE_*`: path input, output, data,
condition, check reference/data term, argument, notifier, term condition),
an edge and a condition index; terms are `SLOT_TERM` handles on the conn.
Their `llg_vpi_timing_t` holds the declared delays or limits in ticks and
the `llg_sdf_op_t` list that addresses the runtime tables; primitives and
continuous assignments carry one too. `vpi_get_delays` reads the live
tables (path delays and `llg_module_path_pulse_limits`, gate triples, check
limits); `vpi_put_delays` builds an annotation from the row's ops and runs
it through `llg_sdf_apply`, so a write behaves as SDF annotation does. Writes
need the tables codegen reserves for a `--vpi`/`--dpi-lib` model
(`llg_vpi_model_delays`, which also passes the min:typ:max selection).
`cbTchkViolation` installs the timing-check violation hook on its first
registration and maps the check back to its row through an index built on
the first violation; the hook is removed at shutdown.
`vpi_put_value` fails in read-only regions without touching the runtime's
fatal region check; zero-delay time callbacks are refused there before
scheduling. `vpi_control` stop/finish use `llg_rt_control_stop/finish`
(levels outside 0-2 report as 1, S43-D8); a stop from the start callbacks uses
`llg_rt_control_stop_before_run`, which under the EXIT policy suspends before
the first `llg_rt_run` (the generated `main` then skips the advance, S43-D10);
`cbEndOfSimulation` runs once, from advance or close.

End-of-step recording, the pending dump and the producer-side dump state
(SIM-047, S47-D1..D8): `llg_wave.h` and `llg_wave.c` comments. Registrations name
static model storage only (signals, record members, fixed arrays); nothing
collectable is registered, so no handle outlives its storage.
The producer/writer threading and fence rules are in the `llg_wave.c` comments.
A second producer needs a separate ring or changed concurrency design.
Only the simulation thread prints waveform reports, each in one stdio call: the
writer stores its single error report and `llg_wave_slot_end`/close print it
(S47-D11). Every other runtime diagnostic is also one stdio call: multi-part
messages (severity tasks, assertion failures, `$finish`/`$stop`, memory-file
diagnostics, the coroutine backtrace, whose frame without a descriptor name, the
shared root of a body class, is named after the running process) are
assembled in an `llg_report_t`
(`llg_rt_prelude.c`; inline buffer, heap growth, truncation instead of abort)
and written with one `fwrite`. Error paths only; `$display` output is untouched.
Separate hierarchy components with ASCII 0x1f and reversibly encode punctuation to
avoid escaped-name collisions. Keep libfst internal parallel mode disabled;
waveform builds require `Threads::Threads`, not C11 threads as the Windows
boundary. Bundled zlib: [readme](readme.md#embedding-and-build); never reintroduce
`find_package(ZLIB)` or a system zlib link. Preserve GTKWave provenance and
official-reader self-tests.
