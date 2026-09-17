# Supplied review counterexamples

These 26 sources came from `test-to-be-added/counterexamples` and remain
unchanged as small, standalone witnesses. The original review explicitly did
not compile or simulate them. Their `NOT EXECUTED` comments describe that
review, not a claim about the current test suite.

The behavioral areas are covered by active regression suites, generally with
larger fixtures and independent exact-output oracles:

| Witnesses | Existing coverage |
| --- | --- |
| `Alias_*`, `Program_*`, `Mailbox_Type_Mismatch` | `sim_review_batch2.rs` |
| `Clocking_Block_Observed_Event`, `Constructor_Layer_Order`, `Factory_Initializer_Receiver`, `Class_Nonpacked_Initializer`, `Property_Not_Unknown`, `Scanner_Delimiters`, `File_Descriptor_Namespaces` | `sim_review_batch3.rs` |
| `Outdated_Queue_Reference`, `Empty_Repetition_Boundary`, `Nonconsecutive_Extra_Occurrence`, `Nested_First_Match`, `Multiclock_Zero_Delay`, `Implication_Local_Transfer`, `Disjoint_Always_Prefixes` | `sim_review_batch4.rs` |
| `Kill_Ancestor`, `Semaphore_Cancel_Head`, `Explicit_Assertion_Action` | `sim_process_control.rs`, `sim_semaphore.rs`, `sim_review_batch2.rs` |
| `Dpi_String_Alias.sv` and `.c` | `sim_dpi.rs` |
| `Vpi_Time_Request.c`, `Vpi_Vector_Request.c` | `sim_vpi.rs` |

The C VPI files are integration fragments without registration or a complete
plugin. `Dpi_String_Alias.c` must be linked with its SV companion. Treat this
directory as preserved review input; run the active suites above for regression
results. See `tests/readme.md` for the suite coverage and validation commands.
