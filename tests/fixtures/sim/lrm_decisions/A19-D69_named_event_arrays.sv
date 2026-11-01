// Decision A19-D69: an unpacked array of named events is a
// vpiNamedEventArray object whose elements are vpiNamedEvent objects.
// vpi_handle_by_name finds the array and each element by name;
// vpi_handle_by_index (one dimension) and vpi_handle_by_multi_index return
// an element, NULL with an error for an index outside the range or a wrong
// index count. The array has vpiName, vpiFullName, vpiSize (the element
// count), vpiRange (leftmost first), vpiNamedEvent (its elements in
// row-major order), vpiModule and vpiScope; it has no value. An element is
// named after its array with its indices (ev[1]); its vpiParent is the
// array, vpiArrayMember is TRUE, vpiIndex is its index in a
// one-dimensional array and the vpiIndex iteration gives its indices
// innermost first. An element is a named event: vpiWaitingProcesses lists
// the threads waiting on it, cbValueChange watches it and vpi_put_value
// triggers it. A scope lists the array under vpiNamedEventArray, never its
// elements under vpiNamedEvent.
//
// IEEE 1800-2009 37.25 (SystemVerilog-1800-2009.txt L59284-59345) draws
// the named event array with "-> access by index vpi_handle_by_index()
// vpi_handle_by_multi_index()", a vpiParent relation from its named events
// and vpiIndex, and its details: "2) vpi_iterate(vpiRange,
// named_event_array_handle) shall return the set of array range
// declarations beginning with the leftmost unpacked range" and "1)
// vpi_iterate(vpiIndex, named_event_handle) shall return the set of indices
// for a named event within an array, starting with the index for the named
// event and working outward." The named event diagram gives a named event
// "-> value vpi_put_value()" and vpiWaitingProcesses. Some simulators
// report the array as a vpiRegArray, list it only under vpiVariables,
// iterate its elements with vpiReg and refuse vpi_put_value and
// vpiWaitingProcesses on an element, and list the indices of an element
// outermost first; llg follows the clause text (37.25 defines
// vpiNamedEventArray, gives every named event a value and waiting processes
// and lists the indices starting with the named event's own index, as
// decision A19-D28 does for arrays of variables).
//
// Build A19-D69_named_event_arrays.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d69()` declared as a system task).
module tb;
    event ev [0:2];
    event ev2 [1:0][0:1];
    event single;
    initial begin
        fork
            begin
                @(ev[1]);
                $display("woke on ev[1] at %0t", $time);
            end
        join_none
        #1 $d69;
        #1 $display("done");
        $finish;
    end
endmodule
