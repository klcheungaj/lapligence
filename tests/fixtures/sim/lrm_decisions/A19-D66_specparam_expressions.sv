// Decision A19-D66: the expression of a specparam.
// IEEE 1800-2009 37.26 (L59345) draws vpiExpr from a spec param to an expr
// and gives a parameter's vpiExpr its default value expression (detail 3);
// the text does not describe a specparam's.
// Decision: vpi_handle(vpiExpr) of a specparam returns the expression of
// its declaration, as for a parameter (decision A19-D32): an operation with
// its operands for `2 + 3`, a constant for a literal; the expression
// evaluates to the specparam's value.
`timescale 1ns/1ns
module tb;
  specify
    specparam tm = 4;
    specparam tsum = 2 + 3;
    specparam tmul = tm * 2;
  endspecify
  initial #1 $d66;
endmodule
