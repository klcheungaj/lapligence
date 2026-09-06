# Next-phase datatype fixture contracts

`sim_data_types_next.rs` runs each fixture in optimized and unoptimized models
and requires its exact `PASS` marker. The bounded inventory has 20 positive
focused cases, one vector-strength rejection, and one explicit unsupported
nonconstant static-initializer case.

Inventory:

- `packed_union.sv`, `unpacked_struct.sv`, `unpacked_union.sv`: member
  layout/aliasing, defaults, writes, and aggregate copy.
- `packed_streaming.sv`, `inside_membership.sv`: packed stream order and
  scalar/range/wildcard set membership.
- `streaming_lhs.sv`: non-divisible streaming slices, wide unpacking,
  in-place aliasing, and one-time lvalue selection.
- `continuous_assignment_strengths.sv`,
  `continuous_assignment_highz_strengths.sv`, and
  `continuous_assignment_vector_strength_rejected.sv`: explicit resolution,
  high-Z endpoints, and scalar-only admission on `wire`/`tri` nets.
- `static_subprogram_storage.sv`, `static_function_executable_assignments.sv`,
  `static_function_runtime_initializer.sv`, and `static_task_nba.sv`: persistent
  storage, per-call execution, declaration initialization, cast materialization,
  and output copy-out across calls/NBAs.
- `dynamic_array.sv`, `associative_array.sv`, and `queue.sv`: allocation,
  copy/resize/delete, keyed traversal, and queue mutation.
- `container_assignment_contexts.sv`: packed queue/dynamic element contexts,
  method-argument conversions, and invalid associative keys.
- `string.sv`, `string_conversions.sv`, `dynamic_string_formatting.sv`, and
  `chandle.sv`: values, conversions, casts/copy/display, and foreign handles;
  string formals remain unsupported.
- `string_return_packed_input.sv`: early automatic string returns with packed
  inputs and implicit function-name string copying.

Runtime-dependent static initializers are rejected explicitly rather than
evaluated on first call. Normative provenance is local IEEE 1800-2009 text:
§§4.4.2.4, 5.7.1, 5.7.2, 6.12.2, 6.14–6.16, 6.21, 7.2–7.3, 7.5–7.10,
10.3.4, 10.8, 11.4.13–11.4.14, 11.6, 13.3.2, 13.4.2, 13.5, 21.2, and
28.11–28.12. These section references are indexed in
`docs/specification/spec-reference-sv.md`. Icarus is only a secondary check.
