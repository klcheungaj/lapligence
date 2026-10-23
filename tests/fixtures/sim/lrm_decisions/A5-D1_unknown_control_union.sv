// Decision A5-D1: a switch whose control is x or z gives every node of its
// network the union of the results with the switch conducting and not
// conducting, as a value and as a strength range: a node with one logic level
// in both cases keeps it (with the range of both strengths), every other node
// is x, or L/H when its other case is high impedance.
//
// IEEE 1800-2009 4.9.5 (SystemVerilog-1800-2009.txt L3558-3570): "A
// conceptually simple technique is to solve the network repeatedly with these
// transistors set to all possible combinations of fully conducting and
// nonconducting transistors. Any node that has a unique logic level in all
// cases has steady-state response equal to this level. All other nodes have
// steady-state response x." 28.12.2 L47645-47652 defines the L, H and
// x strength ranges.
`timescale 1ns/1ns
module tb;
  reg c;
  wire a1, b1;               // St1 to an undriven node: StH
  assign (strong0, strong1) a1 = 1'b1;
  tranif1 s1(a1, b1, c);
  wire a2, b2;               // St1 to We0: 36X
  assign (strong0, strong1) a2 = 1'b1;
  assign (weak0, weak1) b2 = 1'b0;
  tranif1 s2(a2, b2, c);
  wire a3, b3;               // St1 to Pu1: one level, two strengths
  assign (strong0, strong1) a3 = 1'b1;
  assign (pull0, pull1) b3 = 1'b1;
  tranif1 s3(a3, b3, c);
  wire a4, b4;               // through rtranif1: PuH
  assign (strong0, strong1) a4 = 1'b1;
  rtranif1 s4(a4, b4, c);
  wire g, h, k;              // h also reaches St1 through trans: St1
  assign (strong0, strong1) g = 1'b1;
  tranif1 s5(g, h, c);
  tran s6(g, k);
  tran s7(k, h);
  initial begin
    c = 1'bx; #1
    $display("x %v %v | %v %v | %v %v | %v %v | %v %v %v", a1, b1, a2, b2, a3, b3, a4, b4, g, h, k);
    c = 1'bz; #1
    $display("z %v %v | %v %v | %v %v | %v %v | %v %v %v", a1, b1, a2, b2, a3, b3, a4, b4, g, h, k);
    $display("%b %b %b %b %b", b1, b2, b3, b4, h);
    $finish;
  end
endmodule
