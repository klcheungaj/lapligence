// SV 1800-2009 14.11: a cycle delay is not a legal intra-assignment delay of
// an ordinary nonblocking assignment; only a synchronous drive takes `##`.
module tb;
  bit clk;
  int x;
  default clocking cb @(posedge clk);
  endclocking
  initial begin
    x <= ##1 5;
    $finish;
  end
endmodule
