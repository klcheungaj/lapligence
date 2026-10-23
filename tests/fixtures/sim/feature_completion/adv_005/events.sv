// ADV-005: a switch network notifies only the nets whose value changed
// (IEEE 1800-2009 4.9.5, 9.4.2 value changes); a pass that changes the
// drivers twice wakes nothing when the result is unchanged. Every change below
// changes the value, so wakeups on strength-only changes are not exercised.
`timescale 1ns/1ns
module tb;
  wire a, b, c, d;
  reg ra, rc, en;
  assign (strong0, strong1) a = ra;
  assign (pull0, pull1) c = rc;
  tran t1(a, b);
  tran t2(b, c);
  tranif1 t3(c, d, en);
  integer na = 0, nb = 0, nc = 0, nd = 0;
  always @(a) na = na + 1;
  always @(b) nb = nb + 1;
  always @(c) nc = nc + 1;
  always @(d) nd = nd + 1;
  initial begin
    ra = 1'bz; rc = 0; en = 1;
    #1 $display("%0t %v %v %v %v | %0d %0d %0d %0d", $time, a, b, c, d, na, nb, nc, nd);
    ra = 1;
    #1 $display("%0t %v %v %v %v | %0d %0d %0d %0d", $time, a, b, c, d, na, nb, nc, nd);
    ra = 0; ra = 1;
    #1 $display("%0t %v %v %v %v | %0d %0d %0d %0d", $time, a, b, c, d, na, nb, nc, nd);
    en = 0;
    #1 $display("%0t %v %v %v %v | %0d %0d %0d %0d", $time, a, b, c, d, na, nb, nc, nd);
    ra = 1'bz;
    #1 $display("%0t %v %v %v %v | %0d %0d %0d %0d", $time, a, b, c, d, na, nb, nc, nd);
    rc = 1'bz; en = 1;
    #1 $display("%0t %v %v %v %v | %0d %0d %0d %0d", $time, a, b, c, d, na, nb, nc, nd);
    ra = 0;
    #1 $display("%0t %v %v %v %v | %0d %0d %0d %0d", $time, a, b, c, d, na, nb, nc, nd);
    $finish;
  end
endmodule
