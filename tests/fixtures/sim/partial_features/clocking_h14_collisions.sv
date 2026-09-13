module tb;
  timeunit 1ns;
  timeprecision 1ns;

  logic clk = 0;
  logic out = 0;
  logic bus_drive0 = 0;
  logic bus_drive1 = 0;
  wire bus;
  assign bus = bus_drive0;
  assign bus = bus_drive1;

  default clocking cb @(posedge clk);
    output #0 out;
    inout bus;
  endclocking

  initial begin
    ##1 cb.out <= 1;
    cb.out <= 0;
    cb.out <= 1;
    #1 $display("collision t=%0t out=%0d", $time, out);
    ##1 cb.bus <= 1;
    #1 $display("conflict t=%0t bus=%b sample=%b", $time, bus, cb.bus);
    #1 $finish;
  end

  always #2 clk = ~clk;
endmodule
