// ADV-014 limit: an expect statement blocks its process until its attempt
// ends; with a global clocking future sampled value function (IEEE
// 1800-2009 16.9.4) that would wait for the next global clock tick. llg
// rejects it at its location.
module tb;
  logic clk = 1'b0, a = 1'b0;
  global clocking @(posedge clk);
  endclocking
  always #5 clk = ~clk;
  initial begin
    expect (@(posedge clk) $rising_gclk(a));
    $finish;
  end
endmodule
