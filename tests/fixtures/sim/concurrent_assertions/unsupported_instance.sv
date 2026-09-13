// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/unsupported_instance.sv
// IEEE 1800-2009 16.13/16.13.8: named property formal bindings expand through
// the owned body, while unsupported temporal operators remain fail-closed.
module tb;
    logic clk;
    logic signal_a;

    property named_property(value);
        @(posedge clk) value until value;
    endproperty

    bad: assert property (named_property(signal_a));

    initial begin
        clk = 1'b0;
        signal_a = 1'b0;
        $finish(0);
    end
endmodule
