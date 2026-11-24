# Known-issues audit (FIN-001 X1)

Negative fixtures for legal forms that `docs/known_issues.md` lists as
rejected but that had no test. Each stays a located compile-time limit; the
section named in the fixture header owns the intended direction. The Rust
suite `tests/sim_feature_completion/ki_audit.rs` runs each one in both
optimizer modes through `sim_cli::reject_case_with_id`, and
`negative_fixture_sweep` keeps every `neg_*` fixture from generating C.

Clause lines quote `docs/specification/SystemVerilog-1800-2009.txt`.

| Fixture | Section | Clause the form is legal under |
| --- | --- | --- |
| `neg_leaf_string_select` | Records of the leaf form | 7.4.6 L8236-8237: "The result of reading from an array with an invalid index shall return the default uninitialized value for the array element type. Writing to an array with an invalid index shall perform no operation." |
| `neg_leaf_handle_member_element` | Records of the leaf form | 11.4.1 L14476-14479 admits the assignment of a whole value of the element type; the element type is an unpacked structure with a class handle member. |
| `neg_pattern_semaphore` | `%p` of tagged unions and built-in synchronization handles | 21.2.1.7 L36425-36426: "The %p and %0p format specifiers can also be used to print singular expressions, in which case the expression is formatted as an element of an aggregate expression described above." |
| `neg_monitor_computed_handle` | Deferred reports that name class state | 21.2.3 L36516-36518: "each time a variable or an expression in the argument list changes value [...] the entire argument list is displayed at the end of the time step" |
| `neg_strobe_printing_helper` | Deferred reports that name class state | 21.2.2 L36483-36484: "The arguments for this task are specified in exactly the same manner as for the $display system task" |
| `neg_strobe_in_final` | Deferred reports that name class state | 9.2.3 L11258-11259: "No remaining scheduled events shall execute after all final procedures have executed." The text does not say whether a `$strobe` issued by a final procedure prints. |
| `neg_helper_event_handle` | Effectful helpers in runtime-callback evaluators | 9.4.2 L11767-11768: "The execution of a procedural statement can be synchronized with a value change on a net or variable or the occurrence of a declared event." |
| `neg_class_key_find_index` | Class-keyed associative arrays | 7.12.1 L9280: "find_index() returns the indices of all the elements satisfying the given expression." |
| `neg_class_key_call_compound` | Class-keyed associative arrays | 11.4.1 L14479-14480: an assignment operator is a blocking assignment "with the exception that any left-hand index expression is only evaluated once." |
| `neg_future_call_argument` | Global clocking future sampled value function limits | 16.9.4 L22888: "$future_gclk ( expression )"; the text puts no restriction on a function call in the argument beyond the 16.9.3 rules for sampled value functions. |
| `neg_process_status_wait` | Rejected process-handle forms | 9.7 L12614: "The status() function returns the process status, as defined by the state enumeration:"; a wait condition may read any expression (9.4.3). |
| `neg_process_suspend_function` | Rejected process-handle forms | 9.7 L12639: "The suspend() function allows a process to suspend either its own execution or that of another process." |
| `neg_process_try_get` | Rejected process-handle forms | 15.4.6 L20583-20584: "If a message is available and the message type is equivalent to the type of the message variable, the message is retrieved, and the method returns a positive integer." |
| `neg_inside_nested_container` | Set membership items outside packed storage | 11.4.13 L15366-15367: "If an expression in the list is an unpacked array, its elements are traversed by descending into the array until reaching a singular value." |
| `neg_inside_container_call` | Set membership items outside packed storage | 11.4.13 L15366-15367, as above; the item is a function returning a queue. |
| `neg_computed_handle_select_write` | Selects of properties of computed class handles | 11.5.1 L15737: "Bit-selects extract a particular bit from a vector, packed array, packed structure, parameter, or concatenation."; the vector is a property reached through a function result (8.4). |
| `neg_record_real_call_equality` | Record results with real members as equality operands | 11.2.2 L14175: "Aggregate expressions can also be compared with equality or inequality operators." |
| `neg_dynamic_array_action` | Deferred assertion actions with container, event or virtual-interface arguments | 16.4 L21223-21225: "Actual argument expressions that are passed by value use the values of the underlying variables at the instant the deferred assertion expression was evaluated." |
| `neg_event_action` | Deferred assertion actions with container, event or virtual-interface arguments | 16.4 L21223-21225, as above; the argument is a named event. |
| `neg_wait_container_element_property` | Waits on storage without a change notification | 9.4.3 L12028-12029: "The execution of a procedural statement can also be delayed until a condition becomes true. This is accomplished using the wait statement" |
| `neg_process_ref_property` | Rejected process-handle forms | 13.5.2 L19103-19104: "Arguments passed by reference are not copied into the subroutine area, rather, a reference to the original argument is passed to the subroutine." The actual is a class property. |
| `neg_process_ref_member` | Rejected process-handle forms | 13.5.2 L19103-19104: "Arguments passed by reference are not copied into the subroutine area, rather, a reference to the original argument is passed to the subroutine." The actual is a record member. |
| `neg_process_output_expression` | Rejected process-handle forms | 13.4 L18775-18777: "It shall be illegal to call a function with output, inout, or ref arguments in an event expression, in an expression within a procedural continuous assignment, or in an expression that is not within a procedural statement."; the call is in an expression within a procedural statement. |
| `neg_process_status_event` | Rejected process-handle forms | 9.7 L12614 and 9.4.2 L11767-11768, as above; the event expression is a status() call. |
| `neg_process_status_implicit` | Rejected process-handle forms | 9.4.2.2 L11892-11894: "The implicit event_expression, @*, is a convenient shorthand that eliminates these problems by adding all nets and variables that are read by the statement (which can be a statement group) of a procedural_timing_ control_statement to the event_expression." |
| `neg_process_status_continuous` | Rejected process-handle forms | The 1800-2009 text states no rule for a class handle read in a continuous assignment; the industry-standard behaviour rejects it too. |
| `neg_real_array_call_equality` | Real references and real-array expressions outside stable storage | 11.2.2 L14175, as above; the operands are function results. |
| `neg_real_array_call_arm` | Real references and real-array expressions outside stable storage | 11.4.11 L15177-15178: "The conditional operator can be used with nonintegral types (see 6.11.1) and aggregate expressions (see 11.2.2) using the following rules:" |
| `neg_real_array_elementwise_limit` | Real references and real-array expressions outside stable storage | 11.4.11 L15177-15178, as above; each operand has 5000 elements. |
| `neg_delayed_native_record_conditional` | Delayed native continuous writes | 9.4.5 L12123-12124: "An intra-assignment delay or event control shall delay the assignment of the new value to the left-hand side, but the right-hand expression shall be evaluated before the delay, instead of after the delay." |
| `neg_record_call_member_select` | Unpacked records with dense member arrays | The 1800-2009 `primary` production (A.8.4 L69083-69098) has no select after a `function_subroutine_call` (L69089); the frontend accepts the form, and llg serves it in the column form (`record_member_forms`). |
| `neg_record_queue_row_select` | Unpacked records with dense member arrays | 7.4.5 L8172-8173: "A subarray is an array that is an element of another array. As in the C language, subarrays are referenced by omitting indices for one or more array dimensions, always omitting the ones that vary most rapidly." |
| `neg_record_member_array_event_write` | Unpacked records with dense member arrays | 9.4.5 L12123-12124, as above; the target is a whole member array of an element. |
| `neg_record_rows_runtime_index` | Unpacked records with dense member arrays | 7.4.6 L8213: "A single element of a packed or unpacked array can be selected using an indexed name." |
| `neg_record_container_element_to_fixed` | Unpacked records with dense member arrays | 7.10.1 L9024: "Queues shall support the same operations that can be performed on unpacked arrays." |
| `neg_record_ref_member_wait_automatic` | Unpacked records with dense member arrays | 13.5.2 L19103-19104: "Arguments passed by reference are not copied into the subroutine area, rather, a reference to the original argument is passed to the subroutine." 9.4.2 L11767-11768, as above, for the event control on a member. |
| `neg_associative_array_action` | Deferred assertion actions with container, event or virtual-interface arguments | 16.4 L21223-21225, as above; the argument is an associative array. |
| `neg_string_in_function_fork` | Strings declared in a branch of a function | 13.4.4 L18999: "Within a function, a fork-join_none construct may contain any statements that are legal within a task." |
| `neg_checker_const_cast` | Checker limits after ADV-015 | 17.3 L28302-28307: "If an actual argument contains any subexpression that is a const cast or automatic value from procedural code, then the corresponding formal argument shall be used only in static assertion statements (see 16.15.6) or static checker instances within the checker. In such cases, the current value of each such subexpression shall be substituted before sampling the full actual argument, whenever a static assertion statement in the checker or a statically instantiated subchecker is added to the pending procedural assertion queue" |
| `neg_procedural_disable_iff_automatic` | Concurrent property limits after SIM-038 | 16.15.6.1 L26881-26884: "a procedural concurrent assertion shall not sample any const expression or automatic variable, but shall instead save the value of the expression or variable at the time the assertion evaluation attempt is added to the procedural assertion queue." |
| `neg_procedural_clock_iff_const` | Concurrent property limits after SIM-038 | 16.15.6.1 L26881-26884, as above; the const cast is in the clock's `iff`. |
| `neg_rand_assoc_handles` | Constrained randomization limits | 18.4 L29337-29338: "Dynamic arrays, associative arrays and queues can be declared rand or randc. All of the elements in the array are randomized, overwriting any previous data." |
| `neg_rand_assoc_wide_key` | Constrained randomization limits | 18.4 L29337-29338, as above; the index type is 80 bits wide. |
| `neg_rand_assoc_class_key` | Constrained randomization limits | 18.4 L29337-29338, as above; the index type is a class. |
| `neg_rand_nested_queue` | Constrained randomization limits | 18.4 L29337-29338, as above; the elements are queues. |
| `neg_rand_solve_element_index` | Constrained randomization limits | 18.5.9 L30068: "solve_before_primary ::= [ implicit_class_handle . | class_scope ] hierarchical_identifier select"; the restrictions (L30072-30080) do not require a constant index. |
| `neg_rand_inside_row` | Constrained randomization limits | 11.4.13 L15366-15367, as above; the item is a row of a two-dimensional property. |
| `neg_rand_element_mode_packed` | Constrained randomization limits | 18.8 L30690-30691: "For unpacked array variables, random_variable can specify individual elements using the corresponding index." |
| `neg_randomize_event_control` | Constrained randomization limits | 18.6.1 L30402-30403: "The randomize() method returns 1 if it successfully sets all the random variables and objects to valid values; otherwise, it returns 0."; 13.4 L18775-18777, as above, forbids only calls with output, inout or ref arguments in an event expression. |
| `neg_prefix_increment_container_member` | UVM example differences | 11.4.2 L14496-14498: "SystemVerilog includes the C increment and decrement assignment operators ++i, --i, i++, and i--. These do not need parentheses when used in expressions." |
| `neg_clocking_block_sequence` | UVM example differences | 16.17 L27651-27655: "No explicit clocking event is allowed in any property or sequence declaration within the clocking block. All sequence and property declarations within the clocking block are treated as though the clocking event of the clocking block had been written explicitly as the leading clocking event." |

`computed_handle_element` is a positive case: the elements of an unpacked
array property of a computed handle, which the known issues listed as
rejected, read, write and take a compound assignment. Its `.out` follows from
11.4.1 L14479-14480 ("any left-hand index expression is only evaluated
once": `get()` runs once per statement, so `calls` counts 1, 2, 3) and from
the stored values 3, 5 and 7; `pop_front` leaves the queue empty (7.10.2.4).

`process_handle_forms` is a positive case for forms the known issues listed as
rejected. `ref` process formals bound to an element of a fixed array and of a
queue store the calling process there (13.5.2 L19103-19104, as above), so the
first line is `0 1 1` (`ps[0]` stays null). `await()` sits in a fork-join_none
branch of a function (13.4.4 L18999, as above; 9.7 L12636: "The await() task
allows one process to wait for the completion of another process."): the
process `p` ends at 3 ns, so the branch prints `finished at 3`. The
always_comb body reads `p.status()`; its sensitivity holds variables and
selects (9.2.2.2.1 L11184-11185: "The implicit sensitivity list of an
always_comb includes the expansions of the longest static prefix of each
variable or select expression that is read within the block or within any
function called within the block"), and a status change writes no variable,
so `done` keeps the value computed while `p` runs: `done 0` at 5 ns. The
industry-standard behaviour prints the same three lines.

`record_member_forms` is a positive case for record forms the known issues
listed as limits (7.2.2 L7765: "A structure can be assigned as a whole and passed to
or from a subroutine as a whole."). `o.i = x`
copies `a[3] = 7` and `m = 1.5`: `7 1.5`. `f().a[1]` reads element 1 of
`k + 1`: `2` (see the grammar note of `neg_record_call_member_select`).
`r.rows[i] = row` with `i = 2` stores `'{9, "nine"}`: `9 nine`. `oq[0].e[1]`
reads the `'{3, 4}` stored before the push: `3 4`. The two continuous
assignments drive bits 0 and 1 of `arr[1].a` with 1 and 0, printed most
significant first at 1 ns: `01`.

`typename_type_parameter`: 20.6.1 L34538 ("The $typename system function
returns a string that represents the resolved type of its argument.") names
the class the type parameter stands for, with its scope (decision TN-D2).

`vpi_static_initializer` (with `vpi_static_initializer.c`): 36.9.1 L56584
("The registration of system tasks shall occur prior to elaboration or the
resolution of references.") puts the registration of a user system function
before any use, and the application starts before static initialization
(decision S43-D12), so `int y = $sched();` runs and `y` is 7. It pins a known
issue: the delayed put its calltf schedules needs the error of 13.4.4
L18996-18997 ("Implementations shall issue an error either at compile time or
run time when these provisions have not been met."), as for S13-D5, but
matures at time 1, so the run prints `x=5 y=7` (as the industry-standard
behaviour does). A fix of the known issue updates the expected output.
