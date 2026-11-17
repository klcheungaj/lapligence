// FIN-001 A3 limit: a local variable passed to a local output formal of a
// sequence whose end point is read (`s1(v).triggered`, IEEE 1800-2009 16.10,
// 16.13.6) would flow out of an end point evaluated in its own attempt; the
// end point is rejected at the instance.
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0;
  logic [3:0] d = 4'h0;
  sequence s1(local output logic [3:0] o);
    @(posedge clk) (a, o = d) ##1 b;
  endsequence
  property p;
    logic [3:0] v;
    @(posedge clk) s1(v).triggered |-> v == 4'h3;
  endproperty
  c1: assert property (p);
  always #5 clk = ~clk;
  initial #40 $finish;
endmodule
