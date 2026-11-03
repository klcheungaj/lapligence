// Decision A19-D80: an unpacked `event` array property of a class is a
// vpiNamedEventArray object, as a module-level array is (decision A19-D69).
// A class object lists its event array properties, inherited ones first,
// under vpiNamedEventArray; a class definition lists the ones it declares.
// The array has vpiName, vpiSize (the element count, also for a class
// definition's property), vpiRange (its declared range) and vpiNamedEvent
// (its events in declaration order); it has no value and no vpiFullName
// (37.17 detail 25: a property of an object has none). vpi_handle_by_index
// and vpi_handle_by_multi_index (one index) take the declared index; an
// index outside the range gives NULL and an error. An element is a named
// event of the object: named after its array with its index (kev[1]), its
// vpiParent is the array, vpiArrayMember is TRUE, vpiIndex is its declared
// index, vpiWaitingProcesses lists the threads waiting on it and
// vpi_put_value triggers it. A class definition's property has elements
// of no object: its vpiNamedEvent iteration is an error.
//
// IEEE 1800-2009 37.25 (SystemVerilog-1800-2009.txt L59284-59345) draws
// the named event array with "-> access by index vpi_handle_by_index()
// vpi_handle_by_multi_index()" and its details: "2) vpi_iterate(vpiRange,
// named_event_array_handle) shall return the set of array range
// declarations beginning with the leftmost unpacked range"; the named
// event diagram gives a named event "-> value vpi_put_value()" and
// vpiWaitingProcesses. 37.27 and 37.29 (class defn, class obj) list
// vpiNamedEventArray. Some simulators report the array as a vpiRegArray,
// iterate its elements with vpiReg and refuse vpi_put_value and
// vpiWaitingProcesses on an element; llg follows the clause text, as
// decision A19-D69 does for a module-level array.
//
// Build A19-D80_class_event_arrays.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d80()` declared as a system task).
`timescale 1ns/1ns
class K;
    event kev [0:1];
    event ke;
    event kw [3:1];
    task wait_kev1;
        @(kev[1]);
        $display("woke kev[1] at %0t", $time);
    endtask
endclass

class D extends K;
    event dev [2];
endclass

module tb;
    K k;
    D d;
    initial begin
        k = new;
        d = new;
        fork
            k.wait_kev1();
        join_none
        #1 $d80;
        #1 $display("end");
        $finish;
    end
endmodule
