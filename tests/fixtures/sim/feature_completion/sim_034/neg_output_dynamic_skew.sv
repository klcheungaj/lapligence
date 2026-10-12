// SV 1800-2009 14.4: an output skew is a constant expression.
module tb;
  bit clk;
  int n = 1;
  int x;
  clocking cb @(posedge clk);
    output #(n) x;
  endclocking
  initial begin
    cb.x <= 1;
    $finish;
  end
endmodule
