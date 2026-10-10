// A context import in $strobe in a design without DPI exports (SV 4.4.2.9,
// 35.5.3): with no exported subroutine to call, the foreign code cannot write
// SystemVerilog storage, so evaluating it in the Postponed region is legal.
`timescale 1ns / 1ns
module tb;
  import "DPI-C" context function int dpi_twice(input int value);

  int a = 0;

  initial begin
    #1 a = 4;
    $strobe("%0t strobe=%0d", $time, dpi_twice(a));
    a = 5;
    #1 $finish(0);
  end
endmodule
