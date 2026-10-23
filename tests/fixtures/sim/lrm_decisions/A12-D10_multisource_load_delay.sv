// Decision A12-D10: a load port on a net with several sources has one
// interconnect delay per transition, combined from the delays annotated
// from its sources by the INTERCONNECT_MIPD setting, the largest by default:
// the delays 2 (from d1) and 6 (from d2) delay every change by 6.
//
// IEEE 1800-2009 32.4.4 (SystemVerilog-1800-2009.txt L52915-52917): "In the
// case of multisource nets, unique delays can be annotated between each
// source/load pair using the INTERCONNECT construct." The text does not
// say how a simulator that keeps one delay per load port combines them.
`timescale 1ns/1ps
module BUF0 (input a, output y);
  buf b (y, a);
endmodule
module DRV (input a, input en, output y);
  bufif1 b (y, a, en);
endmodule
module tb;
  reg a = 1'b0, en1 = 1'b1, en2 = 1'b0;
  wire m, y;
  DRV d1 (a, en1, m);
  DRV d2 (a, en2, m);
  BUF0 l (m, y);
  initial begin
    #10 $sdf_annotate("A12-D10_multisource_load_delay.sdf");
    #10 a = 1'b1;
    #20 en1 = 1'b0;
    en2 = 1'b1;
    #20 a = 1'b0;
    #20 $finish;
  end
  always @(y) if ($realtime > 0) $display("%0t y=%b", $realtime, y);
endmodule
