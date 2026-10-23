// Decision A9-D1: a data event at the time of the reference event violates
// $hold and $recovery, whichever of the two events comes first in the time
// step (also across a #0); it never violates $setup or $removal.
//
// IEEE 1800-2009 31.3.2 (SystemVerilog-1800-2009.txt L51026-51028): "The
// $hold timing check reports a timing violation in the following case:
// (beginning of time window) <= (timecheck time) < (end of time window)";
// 31.3.1 (L50983-50987): "(beginning of time window) < (timestamp time) <
// (end of time window) The end points of the time window are not part of the
// violation region."
`timescale 1ns/1ns
module su(input d, input c); reg n; specify $setup(d, posedge c, 10, n); endspecify endmodule
module ho(input d, input c); reg n; specify $hold(posedge c, d, 10, n); endspecify endmodule
module rc(input r, input c); reg n; specify $recovery(posedge r, posedge c, 10, n); endspecify endmodule
module rm(input r, input c); reg n; specify $removal(posedge r, posedge c, 10, n); endspecify endmodule
module tb;
  reg c;
  reg [2:0] d, h, r, m;
  su s0(d[0], c); su s1(d[1], c); su s2(d[2], c);
  ho h0(h[0], c); ho h1(h[1], c); ho h2(h[2], c);
  rc r0(r[0], c); rc r1(r[1], c); rc r2(r[2], c);
  rm m0(m[0], c); rm m1(m[1], c); rm m2(m[2], c);
  initial
    $monitor("%0t setup=%b%b%b hold=%b%b%b recovery=%b%b%b removal=%b%b%b", $time,
             s2.n, s1.n, s0.n, h2.n, h1.n, h0.n, r2.n, r1.n, r0.n, m2.n, m1.n, m0.n);
  initial begin
    c = 0; d = 0; h = 0; r = 0; m = 0;
    #100 d[0] = 1; h[0] = 1; r[0] = 1; m[0] = 1;  // an earlier delta than the clock
    #0 d[1] = 1; h[1] = 1; r[1] = 1; m[1] = 1;    // the clock's delta, before it
    c = 1;
    #0 d[2] = 1; h[2] = 1; r[2] = 1; m[2] = 1;    // a later delta
    #10 $finish;
  end
endmodule
