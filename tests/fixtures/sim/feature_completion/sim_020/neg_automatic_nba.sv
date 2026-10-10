// SIM-020 A03: a streaming target cannot make a nonblocking assignment to
// an automatic variable (SV 10.4.2).
module tb;
  initial begin
    automatic byte f;
    automatic byte g;
    {>>{f, g}} <= 16'h0102;
    $finish;
  end
endmodule
