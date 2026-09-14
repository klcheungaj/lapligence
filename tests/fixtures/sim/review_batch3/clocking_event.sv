// Static-review regression; not executed during patch preparation.
`timescale 1ns/1ps
module tb;
  bit clk = 0;
  logic [7:0] d = 0;
  logic [7:0] e = 0;
  clocking cb @(posedge clk);
    input #0 d, e;
  endclocking
  clocking previous @(posedge clk);
    input #1step d;
  endclocking
  always @(posedge clk) begin
    d <= d + 1;
    e <= e + 2;
  end
  initial begin
    #1 clk = 1;
    #1 clk = 0;
    #1 clk = 1;
    #1 clk = 0;
  end
  initial begin
    @(cb);
    // No extra delay is allowed here: the block event publishes the samples.
    if (cb.d !== 1 || cb.e !== 2) $fatal(1, "first Observed samples");
    @(cb);
    if (cb.d !== 2 || cb.e !== 4) $fatal(1, "second Observed samples");
    #1;
    $display("clocking event ok");
    $finish(0);
  end
  initial begin
    @(previous);
    if (previous.d !== 0) $fatal(1, "one-step sample");
  end
endmodule
