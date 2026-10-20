// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/nested_clock_iff.sv
// A nested `iff`-qualified clock inside a sequence (IEEE 1800-2009 16.14,
// 9.4.2.3), rejected before ADV-013, ticks only where its condition holds:
// the edge of clk2 at 10 (en = 0) is no tick, so b is checked at 20, not 10.
`timescale 1ns / 1ns
module tb;
    logic clk = 1'b0;
    logic clk2 = 1'b0;
    logic en = 1'b0;
    logic a = 1'b0;
    logic b = 1'b0;

    nested: assert property (@(posedge clk) a ##1 @(posedge clk2 iff en) b)
        $display("PASS %0t", $time); else $display("FAIL %0t", $time);

    initial begin
        #2 a = 1'b1;
        #3 clk = 1'b1;
        #3 a = 1'b0;
        #2 clk2 = 1'b1;
        #3 clk2 = 1'b0;
        #2 begin
            en = 1'b1;
            b = 1'b1;
        end
        #5 clk2 = 1'b1;
        #1 $finish(0);
    end
endmodule
