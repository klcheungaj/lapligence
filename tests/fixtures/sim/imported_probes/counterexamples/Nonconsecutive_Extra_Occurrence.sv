// Static-review counterexample; NOT EXECUTED.
module tb;
  logic clk = 0;
  bit start = 1, b = 1, c = 0;
  int hits;
  cv: cover property (@(posedge clk) start ##0 b[=1] ##1 c) hits++;
  initial begin
    #1; clk = 1;
    #1; clk = 0; start = 0;  // b is still true at the next sample.
    #1; clk = 1;
    #1; clk = 0; b = 0; c = 1;
    #1; clk = 1;
    #1;
    if (hits != 0) $fatal(1, "nonconsecutive repetition skipped an extra true b");
    $finish(0);
  end
endmodule
