// IEEE 1800-2009 11.4.12: concatenation operands must be integral.
module tb;
  real r;
  logic [63:0] v;
  initial begin
    r = 1.0;
    v = {r};
    $finish(0);
  end
endmodule
