// ADV-005 decision A5-D2: a node passes on only what survives its own
// resolution (IEEE 1800-2009 28.12.1, 4.9.5): St0 and Su1 at one node give
// Su1, so a tran passes St1 and an rtran Pu1, not an unknown.
`timescale 1ns/1ns
module tb;
  wire a, b, c, d;
  wire p, q, r;
  reg r0, r1;
  assign (supply0, supply1) a = r1;
  assign (strong0, strong1) a = r0;
  tran t1(a, b);
  rtran t2(a, c);
  tran t3(b, d);
  assign (strong0, strong1) p = r0;
  assign (supply0, supply1) p = r1;
  rtran t4(p, q);
  assign (pull0, pull1) r = 1'b0;
  rtran t5(q, r);
  initial begin
    r1 = 1; r0 = 0; #1
    $display("A %v %v %v %v | %v %v %v", a, b, c, d, p, q, r);
    r1 = 1'bz; #1
    $display("B %v %v %v %v | %v %v %v", a, b, c, d, p, q, r);
    r1 = 1'bx; #1
    $display("C %v %v %v %v | %v %v %v", a, b, c, d, p, q, r);
    r1 = 0; r0 = 1; #1
    $display("D %v %v %v %v | %v %v %v", a, b, c, d, p, q, r);
    $finish;
  end
endmodule
