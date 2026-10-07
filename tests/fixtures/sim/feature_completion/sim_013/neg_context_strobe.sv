// SIM-013 A03 negative: a context import may write SystemVerilog storage
// through exported subroutines (SV 35.5.3), which is illegal in the
// read-only Postponed region where $strobe evaluates (SV 4.4.2.9).
module tb;
  import "DPI-C" context function int dpi_peek(input int value);
  int a = 0;
  initial begin
    $strobe("%0d", dpi_peek(a));
    #1 $finish(0);
  end
endmodule
