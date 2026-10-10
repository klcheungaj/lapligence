// SIM-013 A03 negative: the design exports `set_a` (SV 35.5.4), so the
// foreign code a context import runs may write SystemVerilog storage through
// it (SV 35.5.3). Writing is illegal in the Postponed region where $strobe
// evaluates (SV 4.4.2.9); llg cannot see into the foreign code, so it rejects
// a context import in $strobe whenever the design has DPI exports.
module tb;
  import "DPI-C" context function int dpi_peek(input int value);
  export "DPI-C" function set_a;
  int a = 0;
  function void set_a(input int value);
    a = value;
  endfunction
  initial begin
    $strobe("%0d", dpi_peek(a));
    #1 $finish(0);
  end
endmodule
