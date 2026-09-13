module tb;
  timeunit 1ns;
  timeprecision 1ns;

  logic clk = 0;
  logic out_negedge = 0;
  logic out_posedge = 0;

  default clocking cb @(posedge clk);
    output negedge out_negedge;
    output posedge out_posedge;
  endclocking

  initial begin
    ##1 cb.out_negedge <= 1;
    ##1 cb.out_posedge <= 1;
    #1 $display("edges t=%0t neg=%0d pos=%0d", $time, out_negedge, out_posedge);
    #1 $finish;
  end

  always #1 clk = ~clk;
endmodule
