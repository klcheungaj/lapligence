// llg loads a whole dynamic array or queue at its current size (IEEE
// 1800-2009 21.4.1) but not a slice of one; the call is rejected before a
// model is built.
module tb;
  logic [7:0] q [$];
  initial begin
    q = '{1, 2, 3};
    $readmemh("in.hex", q[0:1]);
  end
endmodule
