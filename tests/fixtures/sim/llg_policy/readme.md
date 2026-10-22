# llg policy cases

Cases that pin a choice llg makes where the LRM leaves the result open and that
has no portable form. They are run by `tests/sim_llg_policy.rs` on every value
backend in both optimizer modes. They are not part of the portable
[`lrm_decisions`](../lrm_decisions/) set and are not meant to be rerun on
another simulator; the [decision register](../../../../docs/lrm_decisions.md)
names the portable part of each decision.

| Case | Policy | Portable counterpart |
| --- | --- | --- |
| `S14-D1_negative_real_repeat_count` | A negative real repeat count rounds and waits for nothing (loop, event control, intra-assignment). | `lrm_decisions/S14-D1_real_repeat_count` (positive counts) |
| `S16-D9_draw_is_urandom_range` | A `randsequence` rule choice draws the value `$urandom_range(sum - 1, 0)` would return. | none |
| `S29-D6_failed_call_consumes_draws` | A failed `randomize()` keeps the draws it made before it found the constraints infeasible, so the next call does not repeat the values it would have drawn. | `lrm_decisions/S29-D6_failed_call_keeps_values` (the variables keep their values) |
| `S31-D7_draw_is_urandom_range` | A `randcase` draws the value `$urandom_range(sum - 1, 0)` would return. | none |
| `CD-D2_full_scope_names` | `%m` names a block of a class method `C.f.fb` and an `extern` method `C.e`. | `lrm_decisions/CD-D2_named_block_scope` (trailing components and module names) |
| `S99-D10_full_scope_names` | `%m` names a class method below its class and the scope declaring the class (`C.f`, `p.D.g`, `tb.L.k`). | `lrm_decisions/S99-D10_class_method_scope` (trailing components) |
| `S41-D4_null_arguments` | NULL scope, data, key and name arguments of the DPI scope routines return -1 or NULL; `svSetScope` with a non-scope keeps the current scope. | `lrm_decisions/S41-D4_scope_routine_arguments` (without the NULL probes) |

IEEE 1800-2009 clauses of `S14-D1_negative_real_repeat_count`:

- 6.12.2 (`SystemVerilog-1800-2009.txt` L5531-5533): "Real numbers shall be
  converted to integers by rounding the real number to the nearest integer,
  rather than by truncating it."
- 9.4.5 (L12140-12141): "If the repeat count literal, or signed variable
  holding the repeat count, is less than or equal to 0 at the time of
  evaluation, the assignment occurs as if there is no repeat construct."
