// ADV-012 names: the SDF file name is an integral constant (IEEE 1800-2009
// 32.9). cell.sdf adds 4 to the path delay of 1 of each instance it names.
`timescale 1ns/1ps

module BUF (input a, output y);
  buf g (y, a);
  specify
    (a => y) = 1;
  endspecify
endmodule

module tb;
  reg a;
  wire y0, y1, y2, y3;
  localparam EXACT = "cell.sdf";
  localparam [8*12-1:0] PADDED = "cell.sdf";
  BUF u0 (a, y0);
  BUF u1 (a, y1);
  BUF u2 (a, y2);
  BUF u3 (a, y3);
  initial begin
    $sdf_annotate({"cell", ".sdf"}, u0);
    $sdf_annotate(EXACT, u1);
    $sdf_annotate(PADDED, u2);
    a = 0;
    #20 a = 1;
    #20 $finish;
  end
  always @(posedge y0) $display("y0 %0t", $time);
  always @(posedge y1) $display("y1 %0t", $time);
  always @(posedge y2) $display("y2 %0t", $time);
  always @(posedge y3) $display("y3 %0t", $time);
endmodule
