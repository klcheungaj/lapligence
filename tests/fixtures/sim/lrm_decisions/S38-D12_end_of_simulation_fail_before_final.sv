// Decision S38-D12: when the simulation ends, the fail statements of
// attempts that fail at the end (a pending s_eventually, S38-D1) run before
// the final procedures, so a final procedure sees what they did.
//
// IEEE 1800-2009 9.2.3 (SystemVerilog-1800-2009.txt L11251-11252): the final
// procedure "occurs at the end of simulation time and executes without
// delays." (L11256-11259): "Unlike an initial procedure, the final procedure
// does not execute as a separate process; instead, it executes in zero time,
// as a series of function calls from a single process. All final procedures
// shall execute in an arbitrary order. No remaining scheduled events shall
// execute after all final procedures have executed." The end-of-simulation
// failure is decided when the simulation ends, before the final procedures,
// and a fail statement run after them would be a scheduled event executing
// after all final procedures, which the text forbids.
//
// The clock ticks at 5 and 15; k never rises, so the s_eventually attempt
// started at 5 is still pending when $finish is called at 16.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, k = 1'b0;
  int n;
  initial s1: assert property (@(posedge clk) s_eventually k)
    n++;
  else begin
    n++;
    $display("s1 fail action, n=%0d", n);
  end
  initial begin
    #5 clk = 1'b1;
    #5 clk = 1'b0;
    #5 clk = 1'b1;
    #1 $finish;
  end
  final $display("final procedure, n=%0d", n);
endmodule
