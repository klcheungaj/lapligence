// Decision A19-D8: named events through VPI. A cbValueChange callback on a
// vpiNamedEvent runs each time the event is triggered (`->`, `->>` when its
// trigger matures, or vpi_put_value), with a NULL value, before the processes
// the trigger wakes run. vpi_put_value with vpiNoDelay and a NULL value
// triggers the event at once; llg accepts no delay mode for an event and
// reports vpiError for one. vpi_get_value on a named event fails with
// vpiError: it has no value.
//
// IEEE 1800-2009 38.36.1 (SystemVerilog-1800-2009.txt L64747-L64749):
// "cbValueChange callbacks can be placed onto event statements. When the
// event statement is executed, the callback routine will be called. Because
// event statements do not have a value, when the callback routine is called,
// the value field of the s_cb_data structure will be NULL."; 38.34
// (L64239-L64240): "Calling vpi_put_value() on an object of type
// vpiNamedEvent shall cause the named event to toggle. Objects of type
// vpiNamedEvent shall not require an actual value, and the value_p argument
// may be NULL."; 37.25 (L59284-L59310) lists only vpi_put_value() as the value
// access of a named event. The text does not say when the callback runs
// relative to the woken processes, nor what a delayed put does; the order and
// the error are llg choices.
//
// Build A19-D8_named_event_callback.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d8_watch()`, `task $d8_put()` and
// `task $d8_delay()` declared as system tasks).
`timescale 1ns/1ns
module tb;
    event ev;
    int hits = 0;
    always @(ev) begin
        hits++;
        $display("wake %0d at %0d", hits, $time);
    end
    initial begin
        $d8_watch;
        #1 -> ev;
        #1 ->> ev;
        #1 $d8_put;
        #1 $d8_delay;
        #2 $display("hits %0d", hits);
        $finish;
    end
endmodule
