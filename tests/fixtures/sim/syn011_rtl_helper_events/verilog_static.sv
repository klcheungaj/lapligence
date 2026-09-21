// llg-test-fixture: tests/fixtures/sim/syn011_rtl_helper_events/verilog_static.sv
// IEEE 1364-2001 §§9.7.2, 10.3.1, 10.3.2 and 10.3.3: a static Verilog
// function with a private value formal can be evaluated by an event control.
module tb;
    reg [1:0] trigger;
    integer changes;

    function [31:0] classify;
        input [1:0] value;
        begin
            classify = value + 1;
        end
    endfunction

    always @(classify(trigger))
        changes = changes + 1;

    initial begin
        changes = 0;
        trigger = 2'b00;
        #1 trigger = 2'b11;
        #1 $display("verilog_static changes=%0d classify=%0d", changes,
                    classify(trigger));
        $finish(0);
    end
endmodule
