// S26-D26: $value$plusargs whose conversion and variable are of other kinds.
// Run with the plusargs +h=0x1f +d=x +q=? +m=1x +n=12 +t=ab +r=2.5 +s=1.5x.
// IEEE 1800-2009 21.6 (SystemVerilog-1800-2009.txt L37579-37589): "the remain-
// ing characters of the matching plusarg ... shall be converted from a string
// into the format indicated by the format string and stored in the variable
// provided. ... If characters exist in the string available for conversion
// that are illegal for the specified conversion, the variable shall be written
// with the value 'bx."
// The text does not say what a conversion gives for a variable of another kind,
// or how a decimal number has an unknown value. Decision: there is no base
// prefix, so `0x1f` has an illegal character for every conversion; a decimal
// text is unknown only as one x, z or ? digit; %s of a real variable stores the
// characters' packed value; a conversion other than %s finds the plusarg and
// leaves a string variable as it is; a real conversion takes digits, signs, a
// point and an exponent only.
`timescale 1ns/1ps
module tb;
  reg [15:0] w;
  integer i;
  real r;
  string s = "kept";
  initial begin
    if ($value$plusargs("h=%h", w)) $display("h %h", w);
    if ($value$plusargs("h=%d", w)) $display("d %h", w);
    if ($value$plusargs("h=%b", w)) $display("b %h", w);
    if ($value$plusargs("d=%d", i)) $display("x %b", i);
    if ($value$plusargs("q=%d", i)) $display("z %b", i);
    if ($value$plusargs("m=%d", i)) $display("mixed %b", i);
    if ($value$plusargs("t=%s", r)) $display("characters %0.1f", r);
    if ($value$plusargs("n=%d", s)) $display("string %s", s);
    if ($value$plusargs("h=%h", s)) $display("string %s", s);
    if ($value$plusargs("r=%f", i)) $display("real %0d", i);
    if ($value$plusargs("r=%f", s)) $display("string %s", s);
    if ($value$plusargs("s=%f", r)) $display("bad real %0.1f", r);
    $finish;
  end
endmodule
