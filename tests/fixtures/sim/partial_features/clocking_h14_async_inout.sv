module tb;
  timeunit 1ns;
  timeprecision 1ns;

  logic clk = 0;
  tri external = 1'bz;
  wire bus;
  assign bus = external;

  default clocking cb @(posedge clk);
    inout bus;
  endclocking

  initial begin
    #1 cb.bus <= 1'b1;
    #1 $display("edge t=%0t bus=%b sample=%b", $time, bus, cb.bus);
    #2 $display("drive t=%0t bus=%b sample=%b", $time, bus, cb.bus);
    #1 $finish;
  end

  always #2 clk = ~clk;
endmodule
