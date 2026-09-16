// Static-review counterexample; NOT EXECUTED.
module tb;
  bit a = 0, b = 0;
  logic [1:0] q;
  always_comb q[0] = a;
  always_comb q[1] = b;
  initial begin
    #1; a = 1; b = 1;
    #1;
    if (q !== 2'b11) $fatal(1, "disjoint always_comb writes failed");
    $finish(0);
  end
endmodule
