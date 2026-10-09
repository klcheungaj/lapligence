// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/neg_nested_clock_iff.sv
// A nested `iff`-qualified clock inside a sequence is legal (IEEE 1800-2009
// 16.13, 9.4.2.3) but sequence transitions carry only a signal edge, so it
// is rejected explicitly instead of ticking on every edge.
module tb;
    logic clk = 1'b0;
    logic clk2 = 1'b0;
    logic en = 1'b0;
    logic a = 1'b0;
    logic b = 1'b0;

    nested: assert property (@(posedge clk) a ##1 @(posedge clk2 iff en) b);

    initial #10 $finish;
endmodule
