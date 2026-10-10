// Decision S38-D1: when the simulation ends, an evaluation attempt that is
// still pending fails if an unmet obligation is strong, and ends without any
// result (no pass, no fail) if its unmet obligations are weak.
//
// IEEE 1800-2009 16.13.1 (SystemVerilog-1800-2009.txt L24512-24523): the
// strong c1 "returns true if, and only if, ... There exists a subsequent tick
// of posedge clk and c is true at the first such tick", while the weak a1
// needs c only "If there exists a subsequent tick of posedge clk".
// 16.13.21 (L25794-25823) and F.5.3.2 (L71206-71214) call a finite trace
// that has not met its future obligations "Pending" and reserve "Fails" for a
// bad state; llg reports an unmet strong obligation at the end as a failure
// and never turns a weak one into a pass.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, k = 1'b0;
  initial s1: assert property (@(posedge clk) s_eventually k)
    $display("%0t s1 pass", $time); else $display("%0t s1 fail", $time);
  initial w1: assert property (@(posedge clk) always !k)
    $display("%0t w1 pass", $time); else $display("%0t w1 fail", $time);
  initial begin
    #5 clk = 1'b1;
    #5 clk = 1'b0;
    #5 clk = 1'b1;
    #1 $finish;
  end
endmodule
