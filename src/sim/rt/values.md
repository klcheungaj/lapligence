# Runtime: values and native owners

Value ABI numbers, the legacy `sv4_t` layout and the ownership rules (borrowed
inputs, owned results, `SV4_EMPTY`, containing-object destruction, registered
scopes, no cleanup attributes/VLAs/alloca) are in
[value/facade.md](value/facade.md) and [value/ownership.md](value/ownership.md);
scope-index pointer rules are in the `scheduler/value_scopes.c` header.
Never retain struct copies across move/replace/destroy; retain lexical cells only
as long as queued/ref users require them.

UDP row scans: `sv4_udp_eval` (contract in `value/backend.h`) is used only for
definitions above the emitter's dense-index limit. Dense-index evaluation needs no
runtime operation beyond the bridge's `llg_sv4_state` and `sv4_fill`.
`value_sources` embeds `value/udp.c` in facade order. Sequential UDPs need no
runtime operation: the generated model owns their next-state tables, step
functions and per-instance state records.

Keep `llg_value` scheduler/coroutine/waveform-independent, parity with `core::elab`,
X/Z distinction, actual-width arithmetic/conversions and partial-select X fill.
Array- and record-conditional helpers borrow inputs and return independent
owners with leaf defaults. Strength resolution retains both X endpoints, strict
opposite-endpoint dominance, neutral Z and wired-AND/OR tie rules; pull/supply
biases are sources. A highz endpoint makes an X contribution one-sided (L/H).
Net layout from shapes and pools: `llg_net_layout` docs in `llg_rt.h`.
`llg_net_t.strength` (scheduler `nets.c`) is an optional per-bit strength view
for `%v`: the hull of every level choice, published after the value (with a
pending net delay, at the delayed commit), forced bits at strong; nets without
it pay one NULL test per resolution. Query counts ignore X/Z except isunknown. Numeric/time
conversion rules belong to lowering/value APIs, not host int/long assumptions.
Random streams: `llg_random.h`, `llg_rng.c` and `llg_rt.h` comments and the
readme section "Random streams"; keep the `LLG_RNG_V1` text pointer-free.
The constraint solver (`llg_constraint.c`, [readme](constraints/readme.md))
stays independent of every other runtime module (header contract in
`llg_constraint.h`) and draws through a callback.
Keep its BDD recursion bounded by `LLG_RAND_BDD_BITS`, the CDCL fallback
iterative and used only after BDD and rejection fail (it is not uniform,
S29-D10), every budget a named tunable, and verification on the word-level evaluator, separate from the
bit-blasted engines. Only `scheduler/randomize.c` converts packed values.

Strings clone reads, consume expression operands and replace/update mutations.
Stable callback markers belong only to persistent storage; unchanged writes do
not notify and clones carry no callback. Exclude NUL, use ASCII case conversion,
parse decimal prefixes rather than C hex/NaN/infinity in atoreal, and preserve
finite-double round trips in realtoa. Register every owner spanning callbacks;
multi-event contexts adopt each field before any evaluation, even shared pointers.

Containers require paired init/destroy. Preserve resize prefixes/default growth,
queue bounds/discard rules, ordered integral/byte-string associative keys, and
invalid-index default-read/no-op-write semantics. X/Z integral keys: `container/associative_arrays.c`. Allocation overflow/exhaustion is fatal. Descriptor-backed dynamic
array elements use type operations; do not apply packed-only queue/associative
paths to other types. Packed `sort`/`rsort` evaluate each `with` key once, merge-sort
an index permutation without recursion, then permute data and queue element
identities in place; X/Z-key elements stay fixed. Queue refs use LIFO
`llg_ref_scope_t`: retain descriptor/cell
through copy-out/cancellation; snapshot/disconnect removed identities before
structural mutation. Destruction cannot free pinned cells, and ref lists are
borrowed. Alias reads never publish or mutate resolved storage.

Identity-handle elements (SIM-006) copy the handle: events, opaque objects and
borrowed chandles are never duplicated; `LLG_VALUE_PROCESS` handles are counted
through `llg_value_handle_hooks_t` (retain on copy, release on drop, a pop moves
the reference out). The scheduler installs the hooks at init and never clears
them; `event_new` creates scheduler-owned events for new event elements (Table
6-7), freed in `llg_rt_cleanup`, while missing or invalid reads yield null
(Table 7-1). Element locators (`container/value_elements.c`) return borrowed
element pointers valid until the next structural change; callers write members
in place and then call the container's `*_touch`. Associative reads through an
invalid key or of a nonexistent entry warn (SV 7.8.6) unless an explicit default
is set; invalid-key writes warn and do nothing. Activation and class-property
containers run without dependencies or notification.

Recursive equality (SIM-007) `llg_value_equality` returns unequal, equal or
unknown: a known packed mismatch dominates, strings compare bytes, handles
identities and reals numerically; `llg_{dyn,queue}[_value]_equal` compare
whole containers element-wise. `llg_dyn_value_copy_range` and
`llg_dyn_value_merge` serve fixed-array views (slices, ambiguous
conditionals). A descriptor-array NBA (`llg_dyn_value_nba`, `is_values`)
owns a deep copy of its payload from issue to commit or cancellation.
`llg_value_item_{to,from}_{dyn,queue,dyn_value,queue_value}` move a record's
queue or dynamic-array member between its nested slot in a value (null when
empty) and a standalone container; they never notify the slot's owner.

Method result helpers (SIM-019, `container/method_gather.c`; `gather` and the
no-callback rule: `llg_container.h` comment). `llg_method_unique_positions` and `..._value_positions` keep the
first position of each distinct key in one hashed pass (packed keys hash
consistently with `sv4_same`; real keys drop NaNs and keep the first of each
equal run). `llg_*_sort_by_keys` order a receiver by one precomputed key per
element (packed, real or string), stable and O(n log n), applying the
permutation in place by cycles; a key count that differs from the receiver
size warns and leaves the receiver unchanged. `llg_{dyn,queue}_value_copy_from_*`
copy between a descriptor-backed dynamic array and queue.
