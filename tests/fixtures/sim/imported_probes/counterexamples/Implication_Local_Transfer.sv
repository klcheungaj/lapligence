// Static-review counterexample; NOT EXECUTED.
module tb;
  logic clk = 0;
  bit start = 1;
  int d = 5, q = 0, failures;
  property p;
    int v;
    @(posedge clk) (start, v = d) |=> (q == v);
  endproperty
  a: assert property (p) else failures++;
  initial begin
    #1; clk = 1;
    #1; clk = 0; start = 0; d = 99; q = 5;
    #1; clk = 1;
    #1;
    if (failures != 0) $fatal(1, "antecedent local was not transferred to consequent");
    $finish(0);
  end
endmodule
