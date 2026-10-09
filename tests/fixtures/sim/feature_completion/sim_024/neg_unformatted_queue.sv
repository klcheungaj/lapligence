// SIM-024: an unpacked value without a format specification is illegal
// (SV 21.2.1.7).
module tb;
  int q [$];
  initial $display(q);
endmodule
