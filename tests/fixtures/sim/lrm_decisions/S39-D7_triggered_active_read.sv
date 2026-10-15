// Decision S39-D7: `.triggered` read in the Active region of the time step
// in which the sequence matches is still false; it is true from the Observed
// region on (a process resumed by the match sees it) and false again once
// time advances.
//
// IEEE 1800-2009 9.4.4 (SystemVerilog-1800-2009.txt L12076-12078): "The
// triggered status of a sequence is set during the Observed region and
// persists through the remainder of the time step (i.e., until simulation
// time advances)"; 9.4.2.4 L12002-12003: "A process resumes execution
// following the Observed region in which the end point is detected."
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0;
  sequence ab;
    @(posedge clk) a ##1 b;
  endsequence
  always #5 clk = ~clk;
  initial begin
    #2 a = 1'b1;                 // sampled at 5
    #10 b = 1'b1;                // sampled at 15
  end
  always @(posedge clk)
    if ($time == 15) $display("%0t active %0d", $time, ab.triggered);
  initial begin
    @(ab) $display("%0t resumed %0d", $time, ab.triggered);
    #1 $display("%0t later %0d", $time, ab.triggered);
    $finish;
  end
endmodule
