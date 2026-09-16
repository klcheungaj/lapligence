// Static-review counterexample; NOT EXECUTED.
module tb;
  logic clk = 0;
  logic x = 1'bx;
  int passes, failures;
  a: assert property (@(posedge clk) not x)
      passes++; else failures++;
  initial begin
    #1; clk = 1;
    #1;
    if (passes != 1 || failures != 0) $fatal(1, "property not used four-state expression truth");
    $finish(0);
  end
endmodule
