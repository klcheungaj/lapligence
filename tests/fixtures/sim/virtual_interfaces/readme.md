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
| `interface_array_ports`, `interface_array_ports_2d` | Interface-array ports: elements, slices, modports, nested modules and arrays of instances dividing an array; a port, a slice or a descending array as a value of a virtual interface array, matched left to left; the two-dimensional forms | 23.3.3.4 L41512: "An interface port instance shall always be connected to an interface instance or a higher level interface port."; 23.3.3.5 L41531: "matched to the port left index to left index, right index to right index"; 7.6 L8437: "Correspondence between elements is determined by the left-to-right order of elements in each array." Matches industry-standard behaviour (one dimension) |
| `event_members` | An element (constant and run-time index), a row or all of an event array, and an assignment to an event or a `ref` actual: controls, triggers, `.triggered`, `wait_order`, comparisons, assignments and arguments use the instance bound when they execute | 15.5.5 L20868-20871; 25.9 L44930-44931 |
| `null_event_array` | An element of an event array through null ends the run | 25.9 L44894 |
| `neg_event_array_actual` | Explicit error: an element of an event array as a `ref` actual ([known issue](../../../../docs/known_issues.md#event-forms-through-virtual-interfaces)) | |
| `neg_interface_array_2d` | Explicit error: a two-dimensional array of interface instances as a value ([known issue](../../../../docs/known_issues.md#arrays-of-interface-instances-of-several-dimensions-as-values)) | |
