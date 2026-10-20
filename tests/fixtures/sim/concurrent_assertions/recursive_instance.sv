// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/recursive_instance.sv
// IEEE 1800-2009 16.13.17: a recursive named property with its own clock
// (formerly rejected) checks its argument on every tick after the attempt:
// both attempts fail at the second tick, where signal_a is 0. ADV-013's
// fixtures hold the full recursion witnesses.
module tb;
    logic clk = 1'b0;
    logic signal_a = 1'b1;

    property named_property(value);
        @(posedge clk) value and (1'b1 |=> named_property(value));
    endproperty

    always_check: assert property (named_property(signal_a)) else $display("FAIL");

    initial begin
        #5 clk = 1'b1;
        #5 clk = 1'b0;
        #2 signal_a = 1'b0;
        #3 clk = 1'b1;
        #5 clk = 1'b0;
        $finish(0);
    end
endmodule
