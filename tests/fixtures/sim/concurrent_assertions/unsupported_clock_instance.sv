// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/unsupported_clock_instance.sv
// IEEE 1800-2009 16.8/16.9: a named property's declaration clock and
// call-site clock must identify one sampled domain; conflicting edges fail
// closed with the instance source location.
module tb;
    logic clk;
    logic value;

    property negedge_property(x);
        @(negedge clk) x;
    endproperty

    bad: assert property (@(posedge clk) negedge_property(value));

    initial begin
        clk = 1'b0;
        value = 1'b0;
        #1 $finish(0);
    end
endmodule
