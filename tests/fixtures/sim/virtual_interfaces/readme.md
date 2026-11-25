# Virtual interfaces

Run by [`tests/sim_virtual_interfaces.rs`](../../../sim_virtual_interfaces.rs).
Clause lines are quoted from `SystemVerilog-1800-2009.txt`. The outputs below
are the industry-standard behaviour too.

| Fixture | Covers | Clause |
| --- | --- | --- |
| `events` | `->`, `->>`, `->> #d`, `.triggered` and `@` of an interface's event through a variable, an array element and a class property; a rebinding between the trigger's issue and its update does not redirect it | 25.9 L44930-44931: "Once a virtual interface has been initialized, all the components of the underlying interface instance are / directly available to the virtual interface via the dot notation." |
| `null_event` | A trigger through null ends the run before the next statement | 25.9 L44894: "Attempting to use a null virtual interface shall result in a fatal" |
| `null_event_nba` | An event-controlled write through null fails when the update runs, after the statements that follow its issue | 25.9 L44894 |
| `delayed_nba` | `<= #d`, `<= @e` and `<= repeat (n) @e` to a whole member, a select, a real, an element of an array member (`#d`) and a concatenation; the handle and index are those at issue | 10.4.2 L13070: "control (see 9.4.5). If variable_lvalue requires an evaluation, it shall be evaluated at the same time as the" (right-hand side) |
| `instance_arrays` | An array of interface instances initializes, assigns and passes as a fixed array (ascending, descending, property, formal), dynamic array or queue of handles, leftmost element first | 25.9 L44919-44921: "Assignment ( = ) to the following: / Another virtual interface of the same type / An interface instance of the same type" |
| `event_members` | An element (constant and run-time index), a row or all of an event array, and an assignment to an event or a `ref` actual: controls, triggers, `.triggered`, `wait_order`, comparisons, assignments and arguments use the instance bound when they execute | 15.5.5 L20868-20871; 25.9 L44930-44931 |
| `null_event_array` | An element of an event array through null ends the run | 25.9 L44894 |
| `neg_event_array_actual` | Explicit error: an element of an event array as a `ref` actual ([known issue](../../../../docs/known_issues.md#event-forms-through-virtual-interfaces)) | |
