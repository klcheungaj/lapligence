// SIM-020 A03: an associative array type is illegal as a bit-stream cast
// destination (SV 6.24.3).
module tb;
  typedef byte ab_t[int];
  ab_t ab;
  initial begin
    ab = ab_t'(32'h01020304);
    $finish;
  end
endmodule
