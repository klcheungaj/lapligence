// SIM-020 A03: fixed-size bit-stream cast operands of different sizes are a
// compile-time error (SV 6.24.3).
module tb;
  typedef byte b3_t[3];
  b3_t b;
  initial begin
    b = b3_t'(32'h01020304);
    $finish;
  end
endmodule
