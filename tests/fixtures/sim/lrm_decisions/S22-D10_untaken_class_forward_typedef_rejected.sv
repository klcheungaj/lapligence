// Decision S22-D10 (negative): a forward class typedef is not resolved by a
// class of the same name declared in a generate block below its scope, and
// so not by one in an untaken branch: the design fails to compile.
//
// IEEE 1800-2009 8.25 (SystemVerilog-1800-2009.txt L10932-10933): "As with
// other forward typedefs as described in 6.18, the actual class definition
// of a forward class declaration shall be resolved within the same local
// scope or generate block." 27.5 L46364-46366: "The conditional generate
// constructs, if-generate and case-generate, select at most one generate
// block from a set of alternative generate blocks based on constant
// expressions evaluated during elaboration. The selected generate block, if
// any, is instantiated into the model."
module tb;
  typedef class X;
  if (0) begin : x
    class X;
      static int v = 1;
    endclass
  end
  X h;
  initial begin
    $display("h %0d", h == null);
    $finish;
  end
endmodule
