// ADV-014 limit: a sequence end point (`.triggered`, IEEE 1800-2009 16.9.11)
// reports its matches in the time step they occur; with a global clocking
// future sampled value function (16.9.4) a match is known only at the next
// global clock tick. llg rejects it at its location.
module tb;
  logic clk = 1'b0, a = 1'b0;
  global clocking @(posedge clk);
  endclocking
  always #5 clk = ~clk;
  sequence s;
    @(posedge clk) $rising_gclk(a);
  endsequence
  initial begin
    wait (s.triggered);
    $display("triggered");
  end
  initial #20 $finish;
endmodule
