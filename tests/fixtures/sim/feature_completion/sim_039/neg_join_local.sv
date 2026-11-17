// FIN-001 A3 limit: a local variable assigned in one operand of `and` flows
// out of the composite (IEEE 1800-2009 16.10 d), "a local variable that flows
// out of at least one operand shall flow out of the composite sequence unless
// it is blocked"); llg keeps one local array per thread and does not merge
// the operands' locals, so the assignment is rejected at the composite.
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0, c = 1'b0;
  logic [3:0] d = 4'h0, out = 4'h0;
  property p;
    logic [3:0] v;
    @(posedge clk) a |-> ((b ##1 (1, v = d)) and (c ##2 1)) ##1 (out == v);
  endproperty
  a1: assert property (p);
  always #5 clk = ~clk;
  initial #40 $finish;
endmodule
