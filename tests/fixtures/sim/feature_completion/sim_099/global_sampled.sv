// SIM-099 A01: the global-clock sampled-value functions no other fixture
// called, $fell_gclk and $stable_gclk, beside $changed_gclk (IEEE 1800-2009
// 16.9.4). Each compares the sampled value at the global clock tick with the
// sampled value at the previous tick; the first tick is not reported.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0;
  logic v = 1'b0;
  int ticks = 0;
  global clocking @(posedge clk);
  endclocking

  initial begin
    #1 begin
      v = 1'b1;
      clk = 1'b1;
    end
    #1 clk = 1'b0;
    #1 begin
      v = 1'b0;
      clk = 1'b1;
    end
    #1 clk = 1'b0;
    #1 clk = 1'b1;
    #1 clk = 1'b0;
    #1 begin
      v = 1'b1;
      clk = 1'b1;
    end
    #1 $finish;
  end

  always @(posedge clk) begin
    ticks++;
    if (ticks > 1)
      $display("[%0t] fell=%b stable=%b changed=%b", $time, $fell_gclk(v), $stable_gclk(v),
               $changed_gclk(v));
  end
endmodule
