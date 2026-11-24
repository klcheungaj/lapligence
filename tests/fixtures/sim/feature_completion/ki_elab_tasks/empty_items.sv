// A lone `;` is an item of these scopes (IEEE 1800-2009 A.1.4, A.1.7, A.1.8,
// A.1.11 and 27.4) and has no effect.
`timescale 1ns/1ps
package pk;
  ;
  parameter int A = 1;
endpackage
interface ifc;
  ;
  logic a = 1;
endinterface
checker ck(logic c);
  ;
endchecker
program prg;
  ;
endprogram
module tb;
  ;
  function int f(); return pk::A; endfunction;
  ifc fi();
  prg p();
  ck c0(fi.a);
  generate
    ;
    if (1) begin : g ; initial #1 $display("generate %0d", f()); end
    for (genvar I = 0; I < 2; I++) ;
    case (1) 1: ; default: ; endcase
  endgenerate
  initial #2 $finish;
endmodule
