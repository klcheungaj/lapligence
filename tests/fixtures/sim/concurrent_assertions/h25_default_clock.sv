// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/h25_default_clock.sv
// IEEE 1800-2009 14.12/16.13: an assertion with no local clock inherits the
// nearest owned default clocking declaration.
module tb;
    logic clk;
    logic value;

    default clocking cb @(posedge clk); endclocking

    property inherited;
        value |-> value;
    endproperty

    check: assert property (inherited) $display("H25_DEFAULT_CLOCK_PASS");

    initial begin
        clk = 1'b0;
        value = 1'b0;
        #1 value = 1'b1;
        #1 clk = 1'b1;
        #1 clk = 1'b0;
        #1 $finish(0);
    end
endmodule
