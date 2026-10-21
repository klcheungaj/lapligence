# llg policy cases

Cases that pin a choice llg makes where the LRM leaves the result open and that
has no portable form. They are run by `tests/sim_llg_policy.rs` on every value
backend in both optimizer modes. They are not part of the portable
[`lrm_decisions`](../lrm_decisions/) set and are not meant to be rerun on
another simulator; the [decision register](../../../../docs/lrm_decisions.md)
names the portable part of each decision.

| Case | Policy | Portable counterpart |
| --- | --- | --- |
| `S16-D9_draw_is_urandom_range` | A `randsequence` rule choice draws the value `$urandom_range(sum - 1, 0)` would return. | none |
| `S31-D7_draw_is_urandom_range` | A `randcase` draws the value `$urandom_range(sum - 1, 0)` would return. | none |
| `CD-D2_full_scope_names` | `%m` names a block of a class method `C.f.fb` and an `extern` method `C.e`. | `lrm_decisions/CD-D2_named_block_scope` (trailing components and module names) |
| `S99-D10_full_scope_names` | `%m` names a class method below its class and the scope declaring the class (`C.f`, `p.D.g`, `tb.L.k`). | `lrm_decisions/S99-D10_class_method_scope` (trailing components) |
| `S41-D4_null_arguments` | NULL scope, data, key and name arguments of the DPI scope routines return -1 or NULL; `svSetScope` with a non-scope keeps the current scope. | `lrm_decisions/S41-D4_scope_routine_arguments` (without the NULL probes) |
