// Static-review counterexample; NOT EXECUTED.
module tb;
  logic clk = 0;
  bit start = 1, b = 0, c = 1;
  int hits;
  cv: cover property (@(posedge clk) start ##0 b[*0] ##1 c) hits++;
  initial begin
    #1; clk = 1;
    #1;
    if (hits != 1) $fatal(1, "empty ##1 must use the starting tick");
    clk = 0; start = 0; c = 0;
    #1; clk = 1;
    #1; $finish(0);
  end
endmodule
