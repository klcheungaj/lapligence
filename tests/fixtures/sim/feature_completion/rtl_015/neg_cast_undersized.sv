// IEEE 1800-2009 6.24.3: a bit-stream cast between fixed-size types of
// different sizes is an error.
module tb;
  typedef logic [3:0] nib_t [3];
  nib_t n;
  initial begin
    n = nib_t'(16'h1234);
    $finish;
  end
endmodule
