# Runtime functional coverage

`coverage/coverage.c` is the coverage store (IEEE 1800-2009 19). Generated
code owns the static metadata (`llg_cov_point_t`, one `llg_cov_group_t` per
elaborated declaration, one mutable `llg_cov_type_t` per elaborated
declaration, so each module instance has its own type, S44-D1); the runtime owns instance records (`llg_cov_new`), which outlive the
covergroup object and are freed only by `llg_cov_release_all` in
`llg_rt_cleanup`, after `final` procedures (which may still query them) and
after `llg_cov_end_of_run` saves the 19.9 database. Options apply between `llg_cov_new` and `llg_cov_ready`,
which resolves group defaults and sizes counters: dense `uint64_t` arrays up to
`LLG_COV_DENSE_BINS`, otherwise an open-addressed table of hit bins. A sample
(`llg_cov_hit`) is O(1) for automatic bins, O(log n) for enum and explicit
bins, and allocates only when a sparse table grows. Coverage percentages are
computed on demand (19.11). Keep the
store free of process state so a model without covergroups never touches it.
Value bins of one group share a layout (key segments, removed values, the
values each `bins` declaration keeps) unless they depend on covergroup
arguments or `auto_bin_max` differs; `llg_cov_ready` builds it and reports
`detect_overlap` pairs once per layout. Transition bins run compile-time
automata (`llg_cov_trans_t`): one attempt per state with the latest start,
removal by the latest ignore/illegal start, `[]` arrays counted per distinct
sequence in a hash table, and `llg_cov_skip` ending sequences on a disabled
sample. A sample starts with `llg_cov_sample_begin`; `llg_cov_hit` records the
ranks a crossed coverpoint hit and `llg_cov_cross_hit` counts their products.
A cross layout (`llg_cov_xlayout_t`) holds each dimension's value-bin runs with
the select-box signatures, so Bb, Bc and Bu (19.11.2) are counted over
signature classes (at most `LLG_COV_CROSS_CLASSES` combinations) instead of
products; automatic products are dense up to `LLG_COV_DENSE_BINS` and sparse
beyond, so a sample costs the bins it hits. With `merge_instances`, type
coverage is the union of instance bins by 19.11.3 name and a merged bin uses
the largest `at_least` (S45-D18); merged crosses need equal layouts.
Option values that 19.7 lets code write live in the mutable instance and
type records, and queries read them at call time.
`llg_cov_hit`'s borrow contract is in `llg_rt.h`. Its rare paths (transitions, segment walks, the default bin, illegal-value
reports, cross ranks, sparse inserts) stay `LLG_NOINLINE` so a plain
coverpoint's sample keeps a small call frame; inlining them made a sample of
the SIM-044b bench 35% slower.

A clocking event on instance storage runs one waiter per instance, started by
`llg_spawn_owned_with_frame` (`scheduler/process_waits.c`): a runtime-owned
Active process with no parent, fork group or program origin, callable during
initialization, whose random stream is derived from the root by ordinal
without drawing from any stream. It skips its sample while
`llg_cov_sampling` is false. `type_option.strobe` merging and flushing: `llg_cov_strobe` docs in `llg_rt.h`
and `coverage.c`; the sample method then only reads design state.

## Coverage database and assertion coverage (SIM-046)

`coverage/database.c` owns the llg coverage database; its format, atomic load
and save are in the module comment. The checksum covers every preceding byte;
loads read at most `LLG_COV_DB_MAX_BYTES`. Keep the field order when adding fields: a format change bumps
`LLG_COV_DB_VERSION`.

`coverage/assertions.c` holds the assertion coverage items (40.2.2, 40.5.3):
a generated table of `llg_acov_item_t` sorted by statement identity and the
instance table (path, definition, parent), installed by `llg_acov_model`.
Generation emits them only when the design calls a `$coverage_*` function or
the VPI catalog exists, so other models are unchanged and every counting
hook is one NULL test. Concurrent assertions bind their item at registration
(`expect` and end points have none); immediate assertions call
`llg_acov_check`; deferred reports count when they mature or are killed.
`end_assertion_attempts` counts the attempts a disable or kill ends, using
`assertion_attempts_in_progress`. A stopped item counts nothing.
`llg_coverage_call` implements the five 40.3.2 functions with Table 40-1
statuses; the VPI accessors (`llg_acov_item_at`, `llg_coverage_control_vpi`,
`llg_coverage_file_vpi`) are the only way `llg_vpi.c` reaches the items.
An unavailable kind (S46-D5) must never return `SV_COV_OK` from CHECK.

`fsm.c` keeps FSM state coverage (40.4, 40.5.3; A17-D11 to A17-D13). The
generated `llg_fcov_fsm_t` table (state parts as storage bit ranges, states,
a static visit array per FSM) exists only when the model has FSM pragmas
and keeps the coverage tables; `llg_fcov_model` binds it after the storage
initializers, samples each FSM's initial state and adds a value watch
(`llg_value_watch_add`) on every part's storage, so a changing publication
of a state signal resamples its FSMs (`llg_fcov_changed`, a binary search
over the sorted watch keys) and nothing else pays. Visits are static model
storage; transitions are a sorted per-FSM array grown on first sight and
freed at cleanup. `assertions.c` dispatches `SV_COV_FSM_STATE` to
`llg_fcov_scope`; the database's FSM section (bit 4) holds its own schema
hash, the visits and the transitions. `llg_vpi.c` reads the table through
`llg_fcov_count`/`llg_fcov_at`/`llg_fcov_covered_in`.

`code.c` keeps statement and toggle coverage (40.2.2, 40.3, 40.5.3;
S46-D13, S46-D14, A19-D46). Codegen emits the tables only when a
`$coverage_*` call can name the kind (it then starts at time 0) or the VPI
catalog exists (stopped until START), so other models are unchanged.
Statements: each entity is a static `uint64_t` the generated code
increments when the statement starts (`llg_scov_item_t` names it with its
source position, instance and VPI code row); a counter only grows, and
start, stop and reset move per-entity offsets (`base`, `mark`), so a
stopped statement still pays one increment and no branch. Toggles: each
`llg_tcov_signal_t` names a packed storage and its width; the arena holds
per signal the last known value, its known bits, the bits seen at 0 and at
1 and per-bit 0->1 and 1->0 counts. START samples the present value and
adds a value watch (`llg_value_watch_add`, `value_watch_toggle`), so only a
collecting signal pays; STOP removes the watch and RESET clears sightings
and counts but keeps the last value. `llg_tcov_finish` stops every signal
at runtime cleanup. `assertions.c` dispatches `SV_COV_STATEMENT` and
`SV_COV_TOGGLE` to the scope tallies here; the database's statement (bit
8) and toggle (bit 16) sections start with their schema hash (statement
positions; signal names and widths), and a merge validates the whole
section before it adds anything. `llg_vpi.c` reads the counts through
`llg_scov_at_code`/`llg_scov_hits`/`llg_tcov_at_object`/`llg_tcov_bits`
and the `*_covered_in` instance tallies.
