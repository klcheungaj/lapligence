// Decision A5-D2: a switch node passes on only the signal that survives the
// node's own combination: a supply 1 and a strong 0 at one node give Su1, so a
// tran passes St1 and an rtran Pu1 (not an x), and a node driven with x passes
// x at both strengths.
//
// IEEE 1800-2009 4.9.5 (SystemVerilog-1800-2009.txt L3552-3553): "Switch
// processing shall consider all the devices in a bidirectional
// switch-connected net before it can determine the appropriate value for any
// node on the net"; 28.12.1 L47610-47611: "the stronger signal shall dominate
// all the weaker drivers and determine the result"; 28.13 L48297-48298 and
// Table 28-8 L48306-48325 reduce the passed strength.
`timescale 1ns/1ns
module tb;
  wire a, b, c, d;
  reg r0, r1;
  assign (supply0, supply1) a = r1;
  assign (strong0, strong1) a = r0;
  tran t1(a, b);
  rtran t2(a, c);
  tran t3(b, d);
  initial begin
    r1 = 1; r0 = 0; #1
    $display("A %v %v %v %v", a, b, c, d);
    r1 = 1'bz; #1
    $display("B %v %v %v %v", a, b, c, d);
    r1 = 1'bx; #1
    $display("C %v %v %v %v", a, b, c, d);
    $finish;
  end
endmodule
