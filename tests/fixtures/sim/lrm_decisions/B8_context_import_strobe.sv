// B8: a context DPI import called from $strobe in a design without DPI
// exports. Build B8_context_import_strobe.c into a shared library and pass it
// to the simulator.
//
// IEEE 1800-2009 4.4.2.9 (L3212-3214): "No new value changes are allowed to
// happen in the current time slot once the Postponed region is reached.
// Within this region, it is illegal to write values to any net or variable or
// to schedule an event in any previous region within the current time slot."
// 35.5.3 (L55824-55826): "A context imported subroutine, however, can access
// (read or write) any SystemVerilog data objects by calling VPI or by calling
// an export subroutine."
//
// Decision: the text forbids writes in Postponed, not calls of context
// imports. With no export to call, this import cannot write SystemVerilog
// storage, so the call is accepted and evaluated in Postponed (after a = 5).
`timescale 1ns / 1ns
module tb;
  import "DPI-C" context function int b8_twice(input int value);

  int a;

  initial begin
    a = 0;
    #1 a = 4;
    $strobe("%0t: strobe b8_twice(a) = %0d", $time, b8_twice(a));
    a = 5;
    #1 $finish;
  end
endmodule
