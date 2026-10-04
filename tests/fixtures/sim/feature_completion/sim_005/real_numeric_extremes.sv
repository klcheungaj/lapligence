// SIM-005: numeric extremes and the nonfinite conversion policy. Real to
// integral conversion rounds to nearest with ties away from zero and keeps the
// low bits of the target width (IEEE 1800-2009 6.12.2); $rtoi truncates toward
// zero (20.5). Shortreal storage rounds to the nearest IEEE single. A
// nonfinite real converted to an integral type yields 0 in llg (the standard
// leaves the result unspecified).
module tb;
  real r, zero, inf, nan;
  shortreal s;
  int i;
  longint l;
  logic [7:0] b;
  integer j;

  initial begin
    zero = 0.0;
    inf = 1.0 / zero;
    nan = zero / zero;
    i = int'(inf);
    $display("int inf %0d", i);
    i = int'(-inf);
    $display("int -inf %0d", i);
    i = int'(nan);
    $display("int nan %0d", i);
    i = $rtoi(nan);
    $display("rtoi nan %0d", i);
    i = int'(1.0e20);
    $display("int 1e20 %0d", i);
    l = longint'(1.0e20);
    $display("longint 1e20 %0d", l);
    b = byte'(300.6);
    $display("byte 300.6 %0d", b);
    j = integer'(-0.5);
    $display("round -0.5 %0d 0.5 %0d 1.5 %0d -2.5 %0d", j, integer'(0.5), integer'(1.5),
             integer'(-2.5));
    $display("rtoi -1.9 %0d 2.7 %0d", $rtoi(-1.9), $rtoi(2.7));
    s = shortreal'(16777217.0);
    $display("short 2^24+1 %.1f", s);
    s = shortreal'(1.0e40);
    $display("short overflow %h", $shortrealtobits(s));
    s = shortreal'(1.0e-45);
    $display("short subnormal %h", $shortrealtobits(s));
    s = shortreal'(-0.0);
    $display("short -0 %h", $shortrealtobits(s));
    r = $bitstoreal(64'h7fef_ffff_ffff_ffff);
    $display("max %h %0d", $realtobits(r), r * 2.0 == inf);
    r = $bitstoreal(64'h0000_0000_0000_0001);
    $display("min %h %h", $realtobits(r), $realtobits(r * 0.5));
    r = -0.0;
    $display("neg zero %h %0d", $realtobits(r), r == 0.0);
    $display("nan %0d %0d %0d %0d", nan == nan, nan != nan, nan < 1.0, nan > 1.0);
    $display("inf %0d %0d", inf > 1.0e308, -inf < -1.0e308);
    r = $bitstoreal(64'h7ff8_0000_0000_0001);
    $display("payload nan %0d", r == r);
    $finish(0);
  end
endmodule
