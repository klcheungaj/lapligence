// ADV-013 A02 limit: a recursive property with a local variable declaration
// assignment (16.10 L23800-23802: a new copy per evaluation attempt, each
// initialized) re-enters itself while that local is live; rejected with a
// location instead of being approximated.
module tb;
  logic clk = 0, s = 0;
  logic [3:0] cnt = 0;
  property track(x);
    logic [3:0] v = cnt;
    (1'b1 |=> track(x)) and x and (1'b1 |=> (cnt == v + 4'd1));
  endproperty
  p: assert property (@(posedge clk) s |-> track(s || !s));
  initial #20 $finish;
  always #5 clk = ~clk;
endmodule
