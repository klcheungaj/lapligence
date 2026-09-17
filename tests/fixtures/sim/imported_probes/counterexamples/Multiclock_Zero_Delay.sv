// Static-review counterexample; NOT EXECUTED.
module tb;
  logic src_clk = 0, dst_clk = 0;
  int hits;
  cv: cover property (@(posedge src_clk) 1'b1 ##0 @(posedge dst_clk) 1'b1) hits++;
  initial begin #5; src_clk = 1; #2; dst_clk = 1; end
  initial begin
    #8;
    if (hits != 1) $fatal(1, "cross-clock ##0 did not choose the nearest later destination tick");
    $finish(0);
  end
endmodule
