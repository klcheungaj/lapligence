module tb;
  timeunit 1ns;
  timeprecision 1ns;

  logic clk = 0;

  default clocking cb @(posedge clk);
  endclocking

  initial begin
    ##1 $display("body t=%0t", $time);
    #1 $finish;
  end

  always #2 clk = ~clk;
endmodule
