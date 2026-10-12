// SV 1800-2009 14.16: a clockvar is written only by the synchronous drive
// syntax (`<=`); a blocking assignment is an error.
module tb;
  bit clk;
  int x;
  clocking cb @(posedge clk);
    output x;
  endclocking
  initial begin
    cb.x = 1;
    $finish;
  end
endmodule
