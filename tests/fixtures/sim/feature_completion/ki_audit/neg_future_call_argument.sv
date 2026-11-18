// A function call as the argument of a global clocking future sampled value
// function: a located limit (docs/known_issues.md, global clocking future
// sampled value function limits).
module tb;
  logic clk = 0, a = 0;
  int n;
  global clocking @(posedge clk); endclocking
  function automatic logic f(logic v); n++; return v; endfunction
  always #5 clk = ~clk;
  a1: assert property (@(posedge clk) $future_gclk(f(a)) == 0);
  initial begin #30 $display("n=%0d", n); $finish; end
endmodule
