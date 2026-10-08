`timescale 1ns/1ns
// SIM-033: default clocking named by `default clocking id;` (also from a
// nested module), ##0/##N on an irregular clock and global clocking.
module tb;
  logic clk = 1'b0;
  int d = 0;

  always @(posedge clk) d <= d + 1;

  clocking busA @(posedge clk);
    input d;
  endclocking
  default clocking busA;
  global clocking gclk @(negedge clk); endclocking

  module inner;
    default clocking busA;
    initial begin
      ##3;
      $display("inner %0t %0d", $time, busA.d);
    end
  endmodule
  inner i ();

  initial begin
    #2 clk = 1'b1;
    #1 clk = 1'b0;
    #4 clk = 1'b1;
    #2 clk = 1'b0;
    #1 clk = 1'b1;
    #5 clk = 1'b0;
    #10 $finish;
  end

  initial begin
    ##1;
    $display("top %0t %0d", $time, busA.d);
    ##0;
    $display("zero %0t", $time);
    @(gclk);
    $display("global %0t", $time);
  end
endmodule
