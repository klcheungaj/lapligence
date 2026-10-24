# Next-phase datatype fixtures

bounded, non-exhaustive next-phase datatype inventory.

Coverage: untagged and finite packed tagged unions/structs, streaming and
  `inside`, static subprogram storage,
  packed and descriptor-backed dynamic containers (including nested copy,
  resize, delete, and negative-size diagnostics), descriptor-backed real and
  string queues/associative arrays, recursive child-container queue and
  associative storage, executed
  `$typename`/`$bits`/array-query functions, strings, and chandles.
- `syn_015_fixed_stream_contexts.sv` covers fixed bit-stream casts and
  streaming through `const ref`/`ref` formals, nested fixed aggregates,
  selected rows, one-time selected destinations, and overlapping source and
  destination snapshots.
- Its source header follows IEEE 1800-2009 §§6.24.1, 6.24.3, 7.2, 11.4.14,
  and 13.5.2; the section index is
  [`docs/specification/spec-reference-sv.md`](../../../../docs/specification/spec-reference-sv.md).
- Runs: each self-checking fixture runs in both optimizer modes.
- Oracle: exact fixture `PASS` markers are required, except explicit rejection
  cases, which must produce their expected diagnostics.
- Limits: this inventory does not imply exhaustive SystemVerilog compatibility.

## Contracts

- `sim_data_types_next.rs` requires exact `PASS` markers in both optimizer
  modes. Keep positive static-initialization/mixed-lifetime cases separate from
  the vector-strength rejection.
- Runtime-dependent static initializers run once, in the recorded
  edition-specific phase, not lazily on first call.
- Anchors (local IEEE 1800-2009, indexed in `spec-reference-sv.md`): §§4.4.2.4,
  5.7.1, 5.7.2, 6.12.2, 6.14–6.16, 6.21, 7.2–7.3, 7.5–7.10, 10.3.4, 10.8,
  11.4.13–11.4.14, 11.6, 13.3.2, 13.4.2, 13.5, 21.2, 28.11–28.12. Icarus is only
  a secondary check.

| Fixtures | Checks |
| --- | --- |
| `packed_union.sv`, `packed_aggregate_selections.sv`, `unpacked_struct.sv`, `unpacked_union.sv` | Member layout/aliasing, packed dimensions, defaults, writes, aggregate copy |
| `packed_nominal_type_key_default.sv`, `packed_nominal_type_key_uncovered.sv` | A same-width nonmatching packed struct type key falls through to a typed default; without the default the member stays uncovered (§§6.22.1, 10.9.2) |
| `syn_021_tagged_values.sv`, `syn_021_tagged_unpacked_values.sv` | Finite packed tagged-union tags and payloads, void/unequal members, nested values, fixed arrays, ports, zero-time function returns, inactive-member X reads; the unpacked form uses the same finite layout |
| `packed_streaming.sv`, `streaming_general.sv`, `inside_membership.sv` | Packed and fixed/resizable stream order, non-divisible slices, static and runtime `with` selectors, X/Z preservation, scalar/range/wildcard set membership |
| `streaming_lhs.sv` | Non-divisible slices, wide unpacking, in-place aliasing, one-time lvalue selection |
| `continuous_assignment_strengths.sv`, `continuous_assignment_highz_strengths.sv`, `continuous_assignment_vector_strength_rejected.sv` | Explicit resolution, high-Z endpoints, scalar-only admission on `wire`/`tri` nets |
| `static_subprogram_storage.sv`, `static_function_executable_assignments.sv`, `static_function_runtime_initializer.sv`, `mixed_subprogram_lifetimes.sv`, `static_local_multiple_instances.sv`, `static_task_nba.sv` | Persistent storage, per-call execution, runtime declaration initialization, mixed lifetimes, per-instance identity, cast materialization, output copy-out across calls/NBAs |
| `dynamic_array.sv`, `associative_array.sv`, `associative_array_p33.sv`, `queue.sv`, `queue_p32.sv` | Allocation, copy/resize/delete, keyed traversal, associative defaults/wildcard canonicalization, queue mutation, slices, concatenation, bounded retention, `$` indices |
| `associative_array_wildcard_traversal.sv` | Single-fault negative for the §7.8.1 traversal prohibition |
| `container_assignment_contexts.sv` | Packed queue/dynamic element contexts, method-argument conversions, invalid associative keys |
| `string.sv`, `string_conversions.sv`, `dynamic_string_formatting.sv`, `chandle.sv` | Values, conversions, casts/copy/display, typed string formals, locals, returns, foreign handles; automatic string NBA destinations, broader aggregate/continuous paths and unsupported native captures stay restricted |
| `cast_probe.sv`, `bitstream_probe.sv`, `bitstream_containers_probe.sv` | Dynamic cast status/failure, enum validation, static enum coercion, bounded aggregate/container bit-stream order and two-state conversion |
| `string_return_packed_input.sv` | Early automatic string returns with packed inputs, implicit function-name string copying |
| `enum_methods.sv` | Declaration-order first/last/next/prev/num/name, sparse signed values, wrap counts, invalid four-state values, two-state defaults |

Run serially:

```sh
cargo nextest run -E 'test(/^sim_data_types_next::/)' --locked
```
