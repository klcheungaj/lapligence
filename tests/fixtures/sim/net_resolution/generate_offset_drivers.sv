// Bits of a vector net that no driver writes stay z when the other bits are
// driven from loop generate blocks at offset positions (`y[6 + i]`): gates,
// continuous assignments, module output ports, tri and wand nets, and nets
// with a net delay or driven by delayed gates.
`timescale 1ns/1ns
module drv (output [1:0] o, input a);
  assign o = {a, a};
endmodule

module tb;
  logic a = 1'b1, b = 1'b0;
  wire [11:0] y;
  wire [11:0] w;
  wire [11:0] v;
  wire [11:0] u;
  wire [7:0] t;
  tri [7:0] q;
  wand [7:0] wa;
  wire [7:0] #2 d;
  wire [7:0] e;
  for (genvar i = 0; i < 4; i++) begin : l
    buf (y[6 + i], a);
  end
  for (genvar i = 0; i < 4; i++) begin : m
    assign w[2 + i] = a ^ b;
  end
  for (genvar i = 0; i < 2; i++) begin : n
    drv dr (.o(v[4 + 2 * i +: 2]), .a(a));
  end
  for (genvar i = 0; i < 2; i++) begin : k
    buf (u[3 + i], b);
  end
  and (u[10], a, a);
  for (genvar i = 1; i < 4; i++) begin : j
    assign t[i] = a;
  end
  for (genvar i = 0; i < 2; i++) begin : h
    not (q[5 + i], b);
  end
  for (genvar i = 0; i < 2; i++) begin : g
    buf (wa[i], a);
  end
  for (genvar i = 0; i < 2; i++) begin : dl
    buf (d[2 + i], a);
    buf #1 (e[4 + i], a);
  end
  initial begin
    $display("t0 d=%b e=%b", d, e);
    #1;
    $display("y=%b", y);
    $display("w=%b", w);
    $display("v=%b", v);
    $display("u=%b", u);
    $display("t=%b", t);
    $display("q=%b", q);
    $display("wa=%b", wa);
    $display("t1 d=%b e=%b", d, e);
    #2 $display("t3 d=%b e=%b", d, e);
    a = 0;
    #1 $display("t4 d=%b e=%b y=%b", d, e, y);
    #2 $display("t6 d=%b e=%b", d, e);
    $finish(0);
  end
endmodule
