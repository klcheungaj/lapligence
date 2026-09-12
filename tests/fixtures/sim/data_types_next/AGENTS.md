# Next-phase datatype fixture contracts

`sim_data_types_next.rs` runs each fixture in optimized and unoptimized models
and requires its exact `PASS` marker. The bounded inventory has positive
focused cases for static initialization and mixed subprogram lifetimes, plus
one vector-strength rejection.

Inventory:

- `packed_union.sv`, `packed_aggregate_selections.sv`, `unpacked_struct.sv`,
  `unpacked_union.sv`: member layout/aliasing, packed dimensions, defaults,
  writes, and aggregate copy.
- `packed_streaming.sv`, `inside_membership.sv`: packed stream order and
  scalar/range/wildcard set membership.
- `streaming_lhs.sv`: non-divisible streaming slices, wide unpacking,
  in-place aliasing, and one-time lvalue selection.
- `continuous_assignment_strengths.sv`,
  `continuous_assignment_highz_strengths.sv`, and
  `continuous_assignment_vector_strength_rejected.sv`: explicit resolution,
  high-Z endpoints, and scalar-only admission on `wire`/`tri` nets.
- `static_subprogram_storage.sv`, `static_function_executable_assignments.sv`,
  `static_function_runtime_initializer.sv`, `mixed_subprogram_lifetimes.sv`,
  `static_local_multiple_instances.sv`, and `static_task_nba.sv`: persistent
  storage, per-call execution, runtime declaration initialization, mixed
  static/automatic lifetimes, per-instance identity, cast materialization, and
  output copy-out across calls/NBAs.
- `dynamic_array.sv`, `associative_array.sv`, `associative_array_p33.sv`, and
  `queue.sv`: allocation, copy/resize/delete, keyed traversal, associative
  defaults/wildcard canonicalization, and queue mutation. `queue_p32.sv`
  covers queue slices, concatenation, bounded retention, and `$` indices; the
  companion
  `associative_array_wildcard_traversal.sv` is a single-fault negative case for
  the §7.8.1 traversal prohibition.
- `container_assignment_contexts.sv`: packed queue/dynamic element contexts,
  method-argument conversions, and invalid associative keys.
- `string.sv`, `string_conversions.sv`, `dynamic_string_formatting.sv`, and
  `chandle.sv`: values, conversions, casts/copy/display, and foreign handles;
  string formals remain unsupported.
- `cast_probe.sv`, `bitstream_probe.sv`, and `bitstream_containers_probe.sv`:
  dynamic cast status/failure, enum validation, static enum coercion, and
  bounded aggregate/array/container bit-stream order and two-state conversion.
- `string_return_packed_input.sv`: early automatic string returns with packed
  inputs and implicit function-name string copying.

enum_methods.sv covers declaration-order first/last/next/prev/num/name queries,
sparse signed values, wrap counts, invalid four-state values, and two-state
defaults.

Runtime-dependent static initializers are evaluated once in the recorded
edition-specific phase rather than lazily on first call. Normative provenance
is local IEEE 1800-2009 text:
§§4.4.2.4, 5.7.1, 5.7.2, 6.12.2, 6.14–6.16, 6.21, 7.2–7.3, 7.5–7.10,
10.3.4, 10.8, 11.4.13–11.4.14, 11.6, 13.3.2, 13.4.2, 13.5, 21.2, and
28.11–28.12. These section references are indexed in
`docs/specification/spec-reference-sv.md`. Icarus is only a secondary check.
