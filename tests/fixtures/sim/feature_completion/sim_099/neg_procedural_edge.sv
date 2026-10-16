// 16.15.6 c): `edge clk` is the clock the procedure infers, and an llg
// assertion clock is a posedge or negedge.
module tb;
  logic clk = 0, a = 1, q;
  always #5 clk = ~clk;
  always @(edge clk) begin
    q <= a;
    r1: assert property (a);
  end
  initial #12 $finish;
endmodule
