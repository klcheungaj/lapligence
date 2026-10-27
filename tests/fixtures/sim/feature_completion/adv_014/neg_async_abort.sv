// ADV-014 limit: an asynchronous accept_on/reject_on acts between assertion
// clock ticks (IEEE 1800-2009 16.13.14); llg rejects it at its location in an
// assertion with global clocking future sampled value functions (16.9.4).
// The synchronous forms are supported (composition.sv).
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0;
  global clocking @(posedge clk);
  endclocking
  always #5 clk = ~clk;
  a1: assert property (@(posedge clk) accept_on(b) $rising_gclk(a));
  initial #20 $finish;
endmodule
