// SIM-025 A03: "it is illegal to write values to any net or variable ... in
// the Postponed region" (SV 4.4.2.9), so a $strobe argument may not call a
// function that writes design storage.
module tb;
  reg [3:0] a = 1, b = 0;
  function [3:0] bump(input [3:0] v);
    b = v + 4'd1;
    bump = v;
  endfunction
  initial begin
    $strobe("a=%0d", bump(a));
    #1 $finish(0);
  end
endmodule
