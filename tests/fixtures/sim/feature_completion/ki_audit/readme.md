# Known-issues audit (FIN-001 X1)

Negative fixtures for legal forms that `docs/known_issues.md` lists as
rejected but that had no test. Each stays a located compile-time limit; the
section named in the fixture header owns the intended direction. The Rust
suite `tests/sim_feature_completion/ki_audit.rs` runs each one in both
optimizer modes through `sim_cli::reject_case`, and
`negative_fixture_sweep` keeps every `neg_*` fixture from generating C.

Clause lines quote `docs/specification/SystemVerilog-1800-2009.txt`.

| Fixture | Section | Clause the form is legal under |
| --- | --- | --- |
| `neg_leaf_string_select` | Records of the leaf form | 7.4.6 L8236-8237: "The result of reading from an array with an invalid index shall return the default uninitialized value for the array element type. Writing to an array with an invalid index shall perform no operation." |
| `neg_leaf_handle_member_element` | Records of the leaf form | 11.4.1 L14476-14479 admits the assignment of a whole value of the element type; the element type is an unpacked structure with a class handle member. |
| `neg_pattern_semaphore` | `%p` of tagged unions and built-in synchronization handles | 21.2.1.7 L36425-36426: "The %p and %0p format specifiers can also be used to print singular expressions, in which case the expression is formatted as an element of an aggregate expression described above." |
| `neg_monitor_computed_handle` | Deferred reports that name class state | 21.2.3 L36516-36518: "each time a variable or an expression in the argument list changes value [...] the entire argument list is displayed at the end of the time step" |
| `neg_strobe_printing_helper` | Deferred reports that name class state | 21.2.2 L36483-36484: "The arguments for this task are specified in exactly the same manner as for the $display system task" |
| `neg_strobe_in_final` | Deferred reports that name class state | 9.2.3 L11259-11260: "No remaining scheduled events shall execute after all final procedures have executed." The text does not say whether a `$strobe` issued by a final procedure prints. |
| `neg_helper_event_handle` | Effectful helpers in runtime-callback evaluators | 9.4.2 L11767-11768: "The execution of a procedural statement can be synchronized with a value change on a net or variable or the occurrence of a declared event." |
| `neg_class_key_find_index` | Class-keyed associative arrays | 7.12.1 L9280: "find_index() returns the indices of all the elements satisfying the given expression." |
| `neg_class_key_call_compound` | Class-keyed associative arrays | 11.4.1 L14478-14479: an assignment operator is a blocking assignment "with the exception that any left-hand index expression is only evaluated once." |
| `neg_future_call_argument` | Global clocking future sampled value function limits | 16.9.4 L22888: "$future_gclk ( expression )"; the text puts no restriction on a function call in the argument beyond the 16.9.3 rules for sampled value functions. |
| `neg_process_status_wait` | Rejected process-handle forms | 9.7 L12614: "The status() function returns the process status, as defined by the state enumeration:"; a wait condition may read any expression (9.4.3). |
| `neg_process_suspend_function` | Rejected process-handle forms | 9.7 L12639: "The suspend() function allows a process to suspend either its own execution or that of another process." |
| `neg_process_try_get` | Rejected process-handle forms | 15.4.6 L20584: "If a message is available and the message type is equivalent to the type of the message variable, the message is retrieved, and the method returns a positive integer." |
| `neg_inside_nested_container` | Set membership items outside packed storage | 11.4.13 L15366-15367: "If an expression in the list is an unpacked array, its elements are traversed by descending into the array until reaching a singular value." |
| `neg_inside_container_call` | Set membership items outside packed storage | 11.4.13 L15366-15367, as above; the item is a function returning a queue. |
| `neg_computed_handle_select_write` | Selects of properties of computed class handles | 11.5.1 L15737: "Bit-selects extract a particular bit from a vector, packed array, packed structure, parameter, or concatenation."; the vector is a property reached through a function result (8.4). |
| `neg_record_real_call_equality` | Record results with real members as equality operands | 11.2.2 L14175: "Aggregate expressions can also be compared with equality or inequality operators." |

`computed_handle_element` is a positive case: the elements of an unpacked
array property of a computed handle, which the known issues listed as
rejected, read, write and take a compound assignment. Its `.out` follows from
11.4.1 L14478-14479 ("any left-hand index expression is only evaluated
once": `get()` runs once per statement, so `calls` counts 1, 2, 3) and from
the stored values 3, 5 and 7; `pop_front` leaves the queue empty (7.10.2.4).
