# Next-phase datatype fixtures

This directory is the focused datatype conformance inventory.
`tests/sim_data_types_next.rs` runs each fixture in optimized and unoptimized
models and expects its exact `PASS` marker. The inventory is bounded and is
not an exhaustive SystemVerilog-compatibility claim.

| Fixture family | Intended semantic coverage |
| --- | --- |
| `packed_union.sv`, `unpacked_struct.sv`, `unpacked_union.sv` | Member layout/aliasing, defaults, member writes, and aggregate copy |
| `packed_streaming.sv`, `inside_membership.sv` | Packed stream slice order and scalar/range/wildcard set membership |
| `streaming_lhs.sv` | Streaming-target non-divisible slices, wide unpacking, in-place aliasing, and one-time lvalue selection |
| `continuous_assignment_strengths.sv`, `continuous_assignment_highz_strengths.sv`, `continuous_assignment_vector_strength_rejected.sv` | Explicit continuous-assignment resolution, high-Z endpoints, and scalar-only admission on ordinary `wire` and `tri` nets |
| `static_subprogram_storage.sv`, `static_function_executable_assignments.sv`, `static_function_runtime_initializer.sv`, `static_task_nba.sv` | Persistent static function/task storage, per-call procedural and loop-initializer execution, pre-process declaration initialization, explicit size/type-cast materialization, and output copy-out across calls/NBAs |
| `dynamic_array.sv`, `associative_array.sv`, `queue.sv` | Allocation/copy/resize/delete, keyed traversal, and queue mutation methods |
| `container_assignment_contexts.sv` | Packed queue/dynamic element contexts, declared method-argument conversions, and invalid associative keys |
| `string.sv`, `string_conversions.sv`, `dynamic_string_formatting.sv`, `chandle.sv` | String values, declared conversion paths, casts/copy/exact byte display, and null/assignment/comparison foreign handles; string formals remain unsupported |
| `string_return_packed_input.sv` | Early automatic string returns with packed inputs and implicit function-name string copying |

The fixtures use IEEE 1800-2009 constructs. The 22-case inventory comprises
20 positive focused cases, one vector-strength rejection, and one explicit
unsupported nonconstant static-initializer case. Static initializer evaluation
is bounded to constant/provenance-supported forms; runtime-dependent static
initializers are rejected explicitly rather than evaluated on first call.
Final status is bounded to this inventory and does not imply exhaustive
conformance.

Normative provenance is the local IEEE 1800-2009 text: packed/unpacked
structures and unions §§7.2–7.3, resizable containers §§7.5–7.10, strings and
chandles §§6.14–6.16, `inside`/streaming §§11.4.13–11.4.14, scalar
drive-strength resolution §§10.3.4 and 28.11–28.12, string conversions and
formatting §§5.7.2, 6.16, and 21.2, container assignment contexts §§5.7.1,
6.12.2, 10.8, and 11.6, and static subprogram storage/NBA scheduling
§§4.4.2.4, 6.21, 13.3.2, 13.4.2, and 13.5. Icarus is only a secondary check;
its unsupported constructs do not redefine these IEEE-derived oracles.
