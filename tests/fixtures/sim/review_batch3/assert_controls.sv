// Static-review regression; not executed during patch preparation.
`timescale 1ns/1ps
module tb;
  bit clk = 0;
  bit start = 0;
  bit good = 0;
  int failures = 0;
  checked: assert property (@(posedge clk) start |=> good) else failures++;
  initial begin
    #1 start = 1;
    #1 clk = 1;                 // Create one attempt.
    #1 clk = 0; start = 0;
    $assertoff(0, tb.checked);
    #1 clk = 1;                 // Existing attempt must still fail.
    #1 clk = 0;
    if (failures != 1) $fatal(1, "assertoff froze an active attempt");
    $asserton(0, tb.checked);
    start = 1;
    #1 clk = 1;                 // Create an attempt that kill must remove.
    #1 clk = 0;
    $assertkill(0, tb.checked);  // Deliberately NOT followed by assertoff.
    #1 clk = 1;
    #1 clk = 0;
    #1 clk = 1;
    #1 clk = 0;
    if (failures != 1) $fatal(1, "assertkill left checking enabled");
    $asserton(0, tb.checked);
    #1 clk = 1;
    #1 clk = 0; start = 0;
    #1 clk = 1;
    #1;
    if (failures != 2) $fatal(1, "asserton did not restart checking");
    $display("assertion controls ok");
    $finish(0);
  end
endmodule
