// IEEE 1800-2009 6.12, 11.5.1: a real has no bits to select.
module tb;
  real r;
  logic b;
  initial begin
    r = 1.0;
    b = r[0];
    $finish(0);
  end
endmodule
