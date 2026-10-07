// SIM-013 A03: foreign helpers in event expressions and Postponed output.
// Imported functions are legal event-expression operands (SV 9.4.2, 35.5);
// the waiting process calls them, so an import with foreign state is
// evaluated with ordinary call semantics. A non-context import cannot reach
// SystemVerilog storage (SV 35.5.3), so $strobe may call it in Postponed.
`timescale 1ns / 1ns
module tb;
  import "DPI-C" pure function int dpi_twice(input int value);
  import "DPI-C" function int dpi_count(input int value);

  int a = 0;
  int t_count = -1;

  initial #0 forever begin
    @(dpi_twice(a));
    $display("%0t twice=%0d", $time, dpi_twice(a));
  end

  initial begin
    #0 @(dpi_count(a) > 2);
    t_count = $time;
  end

  initial begin
    #1 a = 1;
    #1 a = 1;
    #1 a = 3;
    #1 $strobe("%0t strobe=%0d count=%0d", $time, dpi_twice(a), t_count);
    #1 $finish(0);
  end
endmodule
