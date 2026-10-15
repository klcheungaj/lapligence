// Decision S47-D3: the $dumpvars calls of one time step add their selections
// together; a $dumpvars executed at a later time is ignored with a warning,
// because the header (the variable definitions) is already written.
//
// IEEE 1800-2009 21.7.1.2 (SystemVerilog-1800-2009.txt L37768-37769): "The
// $dumpvars task can be invoked as often as desired throughout the model (for
// example, within various blocks), but the execution of all the $dumpvars
// tasks shall be at the same simulation time." The text does not say what a
// later call does; llg ignores it and warns.
// Compare S47-D3.vcd: it declares tb.a and tb.u.c, not tb.b.
module child;
  logic c;
  initial c = 1'b0;
endmodule

module tb;
  logic a;
  logic b;
  child u();
  initial begin
    a = 1'b0;
    b = 1'b0;
    $dumpfile("S47-D3.vcd");
    $dumpvars(1, tb.a);
    $dumpvars(1, tb.u);
    #1 a = 1'b1;
    $dumpvars(1, tb.b);
    b = 1'b1;
    $strobe("end of step 1: a=%b b=%b c=%b", a, b, u.c);
    #1 $finish;
  end
endmodule
