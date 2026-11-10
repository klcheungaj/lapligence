// An event clocking input is rejected (decision K1-D1).
module tb;
  logic clk = 1'b0;
  event ev;
  clocking cb @(posedge clk);
    input ev;
  endclocking
  initial @(cb.ev) $display("%0t", $time);
endmodule
