// IEEE 1800-2009 23.2.2.2 L40646-40647 (SystemVerilog-1800-2009.txt): "Each port
// declaration provides the complete information about the port. The port's
// direction, width, net or variable type, and signedness are completely
// described."
// L40666-40667: "It is illegal to redeclare any ports of the module in the body
// of the module."
// Decision: an ANSI port redeclared in the body rejects with both locations,
// like any other redeclaration. Some simulators accept it with a warning.
module delayed_input(input i, output o);
  wire #3 i;
  assign o = i;
endmodule

module tb;
  logic x = 0;
  wire o;
  delayed_input u(x, o);
  initial #5 $finish;
endmodule
