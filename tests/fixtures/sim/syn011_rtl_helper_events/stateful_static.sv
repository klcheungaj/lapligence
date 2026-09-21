// llg-test-fixture: tests/fixtures/sim/syn011_rtl_helper_events/stateful_static.sv
// IEEE 1800-2009 §§9.4.2 and 13.4.2: a static function whose return
// variable is read before assignment retains state and is not a read-only
// evaluated event helper.
module tb;
    logic trigger;
    int changes;

    function int stateful(input logic value);
        stateful = stateful + value;
    endfunction

    always @(stateful(trigger))
        changes = changes + 1;

    initial begin
        trigger = 1'b0;
        #1 trigger = 1'b1;
        #1 $display("stateful_static changes=%0d", changes);
        $finish(0);
    end
endmodule
