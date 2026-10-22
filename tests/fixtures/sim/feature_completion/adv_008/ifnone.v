`timescale 1ns/1ns
// ADV-008 A01: ifnone paths. An ifnone path applies when no other path with
// the same terminals is active; an edge-sensitive conditional path whose edge
// did not occur most recently does not block it. Of an ifnone path and an
// unconditional path with the same terminals the ifnone path is kept, and of
// two ifnone declarations of one path the last one.
module ifn(input a, s, output y);
  buf (y, a);
  specify
    if (s) (a => y) = 10;
    ifnone (a => y) = 25;
  endspecify
endmodule

module ifne(input clk, d, c, output q);
  reg r;
  assign q = r;
  always @(clk) r <= d;
  specify
    if (c) (posedge clk => (q +: d)) = 5;
    ifnone (clk => q) = 15;
  endspecify
endmodule

module ifn_dup(input a, output y);
  buf (y, a);
  specify
    ifnone (a => y) = (3, 9);
    ifnone (a => y) = (6, 2);
  endspecify
endmodule

module ifn_uncond(input a, output y);
  buf (y, a);
  specify
    (a => y) = 4;
    ifnone (a => y) = 12;
  endspecify
endmodule

module tb;
  reg a1, s1, clk2, d2, c2, a3, a4;
  wire y1, q2, y3, y4;
  ifn u1(.a(a1), .s(s1), .y(y1));
  ifne u2(.clk(clk2), .d(d2), .c(c2), .q(q2));
  ifn_dup u3(.a(a3), .y(y3));
  ifn_uncond u4(.a(a4), .y(y4));
  initial $monitor("%0t y1=%b q2=%b y3=%b y4=%b", $time, y1, q2, y3, y4);
  initial begin
    a1 = 0; s1 = 1;          // conditional path active: 10 -> 10
    #100 a1 = 1;             // 110
    #100 s1 = 0;
    #50 a1 = 0;              // ifnone 25 -> 275
    #100 s1 = 1'bx;
    #50 a1 = 1;              // x enables the conditional path: 410
  end
  initial begin
    d2 = 0; c2 = 1;
    #5 clk2 = 0;             // q x->0; no posedge yet: ifnone 15 -> 20
    #95 d2 = 1;
    #10 clk2 = 1;            // posedge, c=1: edge path 5 -> 115
    #90 d2 = 0;
    #10 clk2 = 0;            // negedge: only ifnone is latest -> 225
    #90 c2 = 0; d2 = 1;
    #10 clk2 = 1;            // posedge, c=0: ifnone 15 -> 325
  end
  initial begin
    a3 = 0;                  // last ifnone (6, 2): x->0 2 -> 2
    #100 a3 = 1;             // 106
    #100 a3 = 0;             // 202
  end
  initial begin
    a4 = 0;                  // ifnone 12 kept -> 12
    #100 a4 = 1;             // 112
    #100 a4 = 0;             // 212
  end
  initial #500 $finish;
endmodule
