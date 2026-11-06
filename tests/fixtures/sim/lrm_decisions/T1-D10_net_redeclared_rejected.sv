// IEEE 1800-2009 6.5 L4732: "Within a name space (see 3.13), it shall be
// illegal to redeclare a name already declared by a net, variable, or other
// declaration."
// Decision: a redeclaration in a module or generate scope rejects with both
// locations. An ANSI port redeclared in the body is T1-D10's other case.
module tb;
  wire a;
  wire #3 a;
  assign a = 1'b1;
  initial #5 $finish;
endmodule
