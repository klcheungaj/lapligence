// Decision S38-D9: an evaluation attempt that begins on a clock tick where a
// sync_accept_on condition holds is aborted on that tick, like the attempts
// that were already pending.
//
// IEEE 1800-2009 16.13.14 (SystemVerilog-1800-2009.txt L25255-25258): "If
// during the evaluation, the abort condition becomes true, then the overall
// evaluation of the property results in true." L25269-25271: the synchronous
// forms "are evaluated at the simulation time step when the clocking event
// happens"; the attempt that starts on that event is part of the evaluation.
// llg formerly reported only the older attempt.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, a = 1'b0, g = 1'b0;
  a1: assert property (@(posedge clk) sync_accept_on (g) a |=> a)
    $display("%0t a1 pass", $time); else $display("%0t a1 fail", $time);
  initial begin
    a = 1'b1;
    #5 clk = 1'b1;
    #5 clk = 1'b0;
    a = 1'b0;
    g = 1'b1;
    #5 clk = 1'b1;
    #5 clk = 1'b0;
    #1 $finish;
  end
endmodule
