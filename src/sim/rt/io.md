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

VPI handle encoding and lazy catalog indexing: `llg_vpi.c` header and catalog
comments; keep model init O(1). Slots made inside a compiletf/calltf (argument handles and
iterators, element/select arguments) are owned by that invocation and released
when it returns; the call handle itself persists per call site with its user
data. sizetf runs once per registration after the first site's compiletf with no
active call. Retain error records, real catalog width 0 and simulator-owned value
buffers valid until the next query/shutdown. Puts go through `llg_ba`/`llg_ba_d`
(no delay), NBA-region callbacks (delayed modes, `vpiSchedEvent` records) or the
force table (`llg_force_value_parts`, `llg_force_real_value`, release through
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
