// IEEE 1800-2009 11.3.1, Table 11-1: case equality is not a real operator.
module tb;
  real r;
  logic b;
  initial begin
    r = 1.0;
    b = (r === 1.0);
    $finish(0);
  end
endmodule
