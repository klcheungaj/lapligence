// ADV-005 decision A5-D1: a switch with an x or z control gives every node
// the union of its conducting and nonconducting results (IEEE 1800-2009
// 4.9.5; IEEE 1364-2001 5.6.5), with the 28.12.2 strength range of that union.
`timescale 1ns/1ns
module tb;
  // One switch with an unknown control between differently driven nodes.
  reg c, d;
  wire a1, b1;               // St1 -> undriven
  assign (strong0, strong1) a1 = 1'b1;
  tranif1 s1(a1, b1, c);
  wire a2, b2;               // St1 -> We0
  assign (strong0, strong1) a2 = 1'b1;
  assign (weak0, weak1) b2 = 1'b0;
  tranif1 s2(a2, b2, c);
  wire a3, b3;               // St1 -> Pu1
  assign (strong0, strong1) a3 = 1'b1;
  assign (pull0, pull1) b3 = 1'b1;
  tranif1 s3(a3, b3, c);
  wire a4, b4;               // Pu1 -> St0
  assign (pull0, pull1) a4 = 1'b1;
  assign (strong0, strong1) b4 = 1'b0;
  tranif1 s4(a4, b4, c);
  wire a5, b5;               // StX -> undriven
  assign (strong0, strong1) a5 = 1'bx;
  tranif1 s5(a5, b5, c);
  wire a6, b6;               // St1 -> rtranif -> undriven
  assign (strong0, strong1) a6 = 1'b1;
  rtranif1 s6(a6, b6, c);
  wire a7, b7;               // St1 -> St0
  assign (strong0, strong1) a7 = 1'b1;
  assign (strong0, strong1) b7 = 1'b0;
  tranif0 s7(a7, b7, c);
  // Networks where enumerating the unknown switches matters.
  // n: no own driver; x switch to St0; tran to Pu1
  wire a, n, p;
  assign (strong0, strong1) a = 1'b0;
  assign (pull0, pull1) p = 1'b1;
  tranif1 x1(a, n, c);
  tran x2(n, p);
  // two x switches in series between St1 and St1, middle undriven
  wire e, m, f;
  assign (strong0, strong1) e = 1'b1;
  assign (strong0, strong1) f = 1'b1;
  tranif1 x3(e, m, c);
  tranif1 x4(m, f, d);
  // x switch from St1 into a node that also has St1 through a tran
  wire g, h, k;
  assign (strong0, strong1) g = 1'b1;
  tranif1 x5(g, h, c);
  tran x6(g, k);
  tran x7(k, h);
  // x switch from St0 into a node held at Su1: blocked
  wire q, r, t;
  supply1 vdd;
  assign (strong0, strong1) q = 1'b0;
  tranif1 x8(q, r, c);
  tran x9(r, vdd);
  tran x10(r, t);
  // Pu1 own, x switch to St0 own (both sides)
  wire u, v;
  assign (pull0, pull1) u = 1'b1;
  assign (strong0, strong1) v = 1'b0;
  tranif1 x11(u, v, c);
  // x switch between St1 and a node with We0, and a further tran
  wire w, y, z;
  assign (strong0, strong1) w = 1'b1;
  assign (weak0, weak1) z = 1'b0;
  tranif1 x12(w, y, c);
  tran x13(y, z);
  initial begin
    c = 1'bx; d = 1'bx; #1
    $display("x %v %v | %v %v | %v %v | %v %v | %v %v | %v %v | %v %v", a1, b1, a2, b2, a3, b3, a4, b4, a5, b5, a6, b6, a7, b7);
    $display("xx %v %v %v | %v %v %v | %v %v %v | %v %v %v | %v %v | %v %v %v", a, n, p, e, m, f, g, h, k, q, r, t, u, v, w, y, z);
    c = 1'bx; d = 0; #1
    $display("x0 %v %v %v | %v %v %v | %v %v %v | %v %v %v | %v %v | %v %v %v", a, n, p, e, m, f, g, h, k, q, r, t, u, v, w, y, z);
    c = 1'bz; #1
    $display("z %v %v | %v %v | %v %v | %v %v | %v %v | %v %v | %v %v", a1, b1, a2, b2, a3, b3, a4, b4, a5, b5, a6, b6, a7, b7);
    c = 1; #1
    $display("1 %v %v | %v %v | %v %v | %v %v | %v %v | %v %v | %v %v", a1, b1, a2, b2, a3, b3, a4, b4, a5, b5, a6, b6, a7, b7);
    c = 0; #1
    $display("0 %v %v | %v %v | %v %v | %v %v | %v %v | %v %v | %v %v", a1, b1, a2, b2, a3, b3, a4, b4, a5, b5, a6, b6, a7, b7);
    $display("bits %b%b%b%b%b%b%b", b1, b2, b3, b4, b5, b6, b7);
    $finish;
  end
endmodule
