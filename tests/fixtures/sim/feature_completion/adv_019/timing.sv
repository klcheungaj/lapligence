// ADV-019 family 3: module paths and timing checks (IEEE 1800-2009 37.35,
// 37.36), walked by timing.c.
`timescale 1ns/1ps
module dut (input a, input b, input clk, input d, input en, input rst,
             output y, output q);
  reg notifier;
  reg qr;
  buf ib (y, a);
  always @(posedge clk) qr <= d;
  assign q = qr;
  specify
    specparam tpd = 3;
    (a +=> y) = (tpd, 4);
    (posedge clk => (q +: d)) = (5, 6, 7);
    if (en) (b *> y) = 2;
    ifnone (b *> y) = 9;
    $setup(d, posedge clk, 2, notifier);
    $hold(posedge clk, d &&& en, 1);
    $setuphold(posedge rst, b, 3, 1);
    $recovery(posedge rst, en, 4);
    $removal(posedge rst, en, 5);
    $recrem(posedge rst, en, 6, 7);
    $skew(posedge rst, negedge en, 8);
    $timeskew(posedge rst, negedge en, 9);
    $fullskew(posedge rst, negedge en, 10, 11);
    $period(posedge rst, 12);
    $width(negedge rst, 13, 1);
    $nochange(posedge rst, b, 1, 2);
  endspecify
endmodule

module tb;
  reg a = 1'b0, b = 1'b0, clk = 1'b0, d = 1'b0, en = 1'b1, rst = 1'b0;
  wire y, q;
  dut u (.a(a), .b(b), .clk(clk), .d(d), .en(en), .rst(rst), .y(y), .q(q));
  initial begin
    #1 $timing;
    $finish;
  end
endmodule
