// FIN-001 A3 limit: a procedural concurrent assertion reading an automatic
// variable saves its value when queued (IEEE 1800-2009 16.15.6.1); with a
// global clocking future sampled value function (16.9.4) its evaluation is
// deferred to the next global clock tick, which keeps no saved values.
module tb;
  logic clk = 1'b0;
  logic [3:0] a = 4'h0;
  global clocking @(posedge clk);
  endclocking
  always #5 clk = ~clk;
  always @(posedge clk) begin
    automatic logic [3:0] v = a;
    a1: assert property ($future_gclk(a) != v + 4'h2);
  end
  initial #20 $finish;
endmodule
