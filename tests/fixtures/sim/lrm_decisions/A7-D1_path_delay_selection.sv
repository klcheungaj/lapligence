// Decision A7-D1: a module path destination transition is scheduled at the
// larger of (a) the time the distributed delays inside the module deliver it
// and (b) the time of the latest transition of an input with a path to it
// plus that path's delay for the transition. Among paths whose inputs
// transitioned at the same latest time, the smallest delay is used. A net
// delay declared on the destination applies after that, so it adds to the
// path delay: qn rises 2 + 4 time units after D. Every destination with a
// driver starts at x (A7-D6). This is the industry-standard behaviour; llg
// previously counted the destination's net delay as a distributed delay.
//
// IEEE 1800-2009 30.5.3 (SystemVerilog-1800-2009.txt L50166-50173): "The
// simulator shall do this by first determining which specify paths to the
// output are active. Active specify paths are those whose input has
// transitioned most recently in time, ..." and "... comparing the correct
// delay for the specific transition being scheduled from each specify path
// and choosing the smallest."
// IEEE 1800-2009 30.6 (L50201-50202): "If a module contains module path delays
// and distributed delays (delays on primitive instances within the module),
// the larger of the two delays for each path shall be used."
// The text does not say how a net delay on the destination combines with
// the path delay.
`timescale 1ns/1ns
module sel(input A, input B, output Y);
  assign Y = A | B;
  specify
    (A => Y) = (6, 9);
    (B => Y) = (5, 11);
  endspecify
endmodule

module short_chain(input D, output Q);
  wire n;
  buf #0 g1(n, D);
  buf #1 g2(Q, n);
  specify
    (D => Q) = 22;
  endspecify
endmodule

module long_chain(input D, output Q);
  wire n;
  buf #10 g1(n, D);
  buf #20 g2(Q, n);
  specify
    (D => Q) = 22;
  endspecify
endmodule

module net_delay(D, Q);
  input D;
  output Q;
  wire #4 Q;
  assign Q = D;
  specify
    (D => Q) = 2;
  endspecify
endmodule

module tb;
  reg A, B, D;
  wire Y, qs, ql, qn;
  sel s(.A(A), .B(B), .Y(Y));
  short_chain u1(.D(D), .Q(qs));
  long_chain u2(.D(D), .Q(ql));
  net_delay u3(.D(D), .Q(qn));
  initial begin
    $monitor("%0t A=%b B=%b Y=%b D=%b qs=%b ql=%b qn=%b", $time, A, B, Y, D, qs, ql, qn);
    A = 1'b0; B = 1'b0; D = 1'b0;
    #100 A = 1'b1;
    #100 A = 1'b0;
    #100 B = 1'b1;
    #100 B = 1'b0;
    #100 A = 1'b1; B = 1'b1;
    #100 A = 1'b0; B = 1'b0;
    #100 A = 1'b1;
    #100 B = 1'b1;
    #100 A = 1'b0;
    #100 B = 1'b0;
    #100 D = 1'b1;
    #100 $finish;
  end
endmodule
