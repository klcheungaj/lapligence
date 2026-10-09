// SIM-024: an unpacked array has no integral value for `%d`; only `%p`
// formats it (SV 21.2.1.7).
module tb;
  int a [2];
  initial $display("%d", a);
endmodule
