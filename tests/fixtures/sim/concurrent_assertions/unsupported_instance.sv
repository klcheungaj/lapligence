// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/unsupported_instance.sv
// IEEE 1800-2009 16.13/16.13.8: formal bindings are retained in the owned
// assertion graph while named property expansion remains outside H20's
// single-cycle executable subset.
module tb;
    logic clk;
    logic signal_a;

    property named_property(value);
        @(posedge clk) value |-> value;
    endproperty

    bad: assert property (named_property(signal_a));

    initial begin
        clk = 1'b0;
        signal_a = 1'b0;
        $finish(0);
    end
endmodule
