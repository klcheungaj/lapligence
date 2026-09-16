// Static-review counterexample; NOT EXECUTED.
module tb;
  logic clk = 0;
  bit start = 1, a = 1, b = 0, c = 0, d = 1, e = 0;
  int hits;
  cv: cover property (@(posedge clk)
    start ##0 ((first_match(a ##[1:2] b) ##1 c) or (d ##3 e))) hits++;
  initial begin
    #1; clk = 1;
    #1; clk = 0; start = 0; a = 0; d = 0; b = 1;
    #1; clk = 1; // The left first_match ends; its suffix will fail.
    #1; clk = 0; b = 0;
    #1; clk = 1; // c is false.
    #1; clk = 0; e = 1;
    #1; clk = 1; // The independent right branch succeeds.
    #1;
    if (hits != 1) $fatal(1, "nested first_match pruned an unrelated alternative");
    $finish(0);
  end
endmodule
