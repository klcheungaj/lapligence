// IEEE 1800-2009 23.2.2.2 L40646-40647 (SystemVerilog-1800-2009.txt): "Each port
// declaration provides the complete information about the port. The port's
// direction, width, net or variable type, and signedness are completely
// described."
// L40666-40667: "It is illegal to redeclare any ports of the module in the body
// of the module."
// Decision: an ANSI port redeclared in the body rejects with both locations,
// like any other redeclaration. Some simulators accept it with a warning.
module signedness(input [3:0] a, output integer b);
  wire signed [3:0] a;
  always @(a) b = a;
endmodule

module tb;
  logic [3:0] x = 4'b1010;
  integer b;
  signedness u(x, b);
  initial #5 $finish;
endmodule
