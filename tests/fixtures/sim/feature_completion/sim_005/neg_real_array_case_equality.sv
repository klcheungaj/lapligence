// IEEE 1800-2009 11.3.1, Table 11-1: case equality is not a real operator,
// also element by element over real arrays.
module tb;
  real a[2], b[2];
  logic e;
  initial begin
    a = '{1.0, 2.0};
    b = a;
    e = (a === b);
    $finish(0);
  end
endmodule
