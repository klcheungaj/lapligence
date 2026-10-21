// Decision S38-D1: when the simulation ends, an evaluation attempt that is
// still pending fails if it waits on an s_eventually obligation, and ends
// without any result (no pass, no fail) otherwise: with weak obligations
// and with the other strong ones (s_nexttime, s_until, a strong sequence).
//
// IEEE 1800-2009 16.13.1 (SystemVerilog-1800-2009.txt L24512-24523): the
// strong c1 "returns true if, and only if, ... There exists a subsequent tick
// of posedge clk and c is true at the first such tick", while the weak a1
// needs c only "If there exists a subsequent tick of posedge clk".
// 16.13.21 (L25794-25823) and F.5.3.2 (L71206-71214) call a finite trace
// that has not met its future obligations "Pending" and reserve "Fails" for a
// bad state, and do not say what a simulator reports. llg follows the
// industry-standard behaviour: a pending s_eventually is reported as a
// failure, every other pending attempt ends without a result (A13-D7).
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, k = 1'b0;
  initial s1: assert property (@(posedge clk) s_eventually k)
    $display("%0t s1 pass", $time); else $display("%0t s1 fail", $time);
  initial w1: assert property (@(posedge clk) always !k)
    $display("%0t w1 pass", $time); else $display("%0t w1 fail", $time);
  initial n1: assert property (@(posedge clk) s_nexttime [3] k)
    $display("%0t n1 pass", $time); else $display("%0t n1 fail", $time);
  initial u1: assert property (@(posedge clk) !k s_until k)
    $display("%0t u1 pass", $time); else $display("%0t u1 fail", $time);
  initial q1: assert property (@(posedge clk) strong(1'b1 ##2 k))
    $display("%0t q1 pass", $time); else $display("%0t q1 fail", $time);
  initial begin
    #5 clk = 1'b1;
    #5 clk = 1'b0;
    #5 clk = 1'b1;
    #1 $finish;
  end
endmodule
