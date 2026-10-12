// SV 1800-2009 14.11, 14.16: `cb.x <= ##1 v` counts cb's own events, but a
// procedural `##1` prefix needs a default clocking, which this module lacks.
module tb;
  bit clk;
  int x;
  clocking cb @(posedge clk);
    output x;
  endclocking
  initial begin
    ##1 cb.x <= 1;
    $finish;
  end
endmodule
