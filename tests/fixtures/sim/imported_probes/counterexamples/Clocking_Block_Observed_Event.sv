// Static-review counterexample; NOT EXECUTED.
module tb;
  logic clk = 0;
  logic d = 0;
  clocking cb @(posedge clk);
    input #0 d;
  endclocking
  always @(posedge clk) d <= 1;
  initial begin #1; clk = 1; end
  initial begin
    @(cb);
    // There is deliberately no intervening delay here.
    if (cb.d !== 1'b1) $fatal(1, "clocking event preceded input sample publication");
    $finish(0);
  end
endmodule
