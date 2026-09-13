// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/h25_unsupported_multiclock.sv
// IEEE 1800-2009 16.14: a cross-clock sequence boundary with a delay
// greater than one is outside the bounded multiclock subset and must reject.
module tb;
    logic clk_a;
    logic clk_b;
    logic first;
    logic second;

    property unsupported;
        @(posedge clk_a) first ##2 @(posedge clk_b) second;
    endproperty

    assert property (unsupported);
endmodule
