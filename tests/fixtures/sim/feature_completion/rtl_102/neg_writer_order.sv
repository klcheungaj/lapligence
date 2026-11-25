// IEEE 1800-2009 9.2.2.2: every cell of `m` is written by both processes; the
// message names the first.
module tb;
  logic [7:0] m[8];
  always_comb begin
    m[0] = 1; m[1] = 1; m[2] = 1; m[3] = 1; m[4] = 1; m[5] = 1; m[6] = 1; m[7] = 1;
  end
  always_comb begin
    m[7] = 2; m[6] = 2; m[5] = 2; m[4] = 2; m[3] = 2; m[2] = 2; m[1] = 2; m[0] = 2;
  end
  initial $finish(0);
endmodule
