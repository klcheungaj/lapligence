// Decision S47-D4: $dumpvars(levels) without a scope list applies the levels
// below every top-level module instance, wherever it is called, as
// $dumpvars without arguments dumps the whole model; $dumpvars(0) dumps the
// whole model.
//
// IEEE 1800-2009 21.7.1.2 (SystemVerilog-1800-2009.txt L37786): "When invoked
// with no arguments, $dumpvars dumps all the variables in the model to the
// VCD file." L37788-37789: "the first argument indicates how many levels of
// the hierarchy below each specified module instance to dump to the VCD
// file." Syntax 21-15 allows `$dumpvars(levels)` with no module named; the
// text does not say which instances it counts from. llg counts from the top.
// Compare S47-D4.vcd: it declares tb.t only, not tb.u.c.
module child;
  logic c;
  initial begin
    c = 1'b0;
    $dumpfile("S47-D4.vcd");
    $dumpvars(1);
    #1 c = 1'b1;
  end
endmodule

module tb;
  logic t;
  child u();
  initial begin
    t = 1'b0;
    #1 t = 1'b1;
    $strobe("end of step 1: t=%b c=%b", t, u.c);
    #1 $finish;
  end
endmodule
