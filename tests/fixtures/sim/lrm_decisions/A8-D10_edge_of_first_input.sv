// Decision A8-D10: the edge identifier of a full connection with several
// inputs applies to the first input only; the paths from the other inputs of
// the list are active on any transition. The condition, the delays and the
// data source are those of the declaration.
//
// IEEE 1800-2009 30.4.3 (SystemVerilog-1800-2009.txt L49480-49482):
//   "full_edge_sensitive_path_description ::=
//    ( [ edge_identifier ] list_of_path_inputs [ polarity_operator ] *>
//    ( list_of_path_outputs [ polarity_operator ] : data_source_expression ) )"
// L49488-49491: "The edge identifier may be one of the keywords posedge,
//   negedge, or edge, associated with an input terminal descriptor" / "If the
//   edge transition is not specified, the path shall be considered active on
//   any transition at the input terminal."
// The text associates the edge with one terminal and does not say which one
// of a list it is.
`timescale 1ns/1ns
module first(input a, b, c, output y);
  assign y = a ^ b ^ c;
  specify (posedge a, b, c *> (y : a)) = 5; endspecify
endmodule
module mixed(input a, b, c, output y);
  assign y = a ^ b ^ c;
  specify (negedge a, b *> (y : a)) = 7; (c *> y) = 3; endspecify
endmodule
module conditional(input a, b, c, output y, z);
  assign y = a ^ b ^ c;
  assign z = ~(a ^ b ^ c);
  specify
    if (c) (posedge a, b *> (y, z : a)) = 6;
    ifnone (a, b, c *> y, z) = 2;
  endspecify
endmodule
module vector(input [1:0] a, b, output y);
  assign y = ^a ^ ^b;
  specify (posedge a, b *> (y : a)) = 9; endspecify
endmodule
module both(input a, b, output y);
  assign y = a ^ b;
  specify (posedge a, b *> (y : a)) = 5; (posedge b, a *> (y : a)) = 9; endspecify
endmodule
module wide(input a, b, output [1:0] y);
  assign y = {a ^ b, a ~^ b};
  specify (posedge b, a *> (y : a)) = 4; endspecify
endmodule
module tb;
  reg a, b, c;
  reg [1:0] va, vb;
  wire y1, y2, y3, z3, y4, y5;
  wire [1:0] y6;
  first u1(a, b, c, y1);
  mixed u2(a, b, c, y2);
  conditional u3(a, b, c, y3, z3);
  vector u4(va, vb, y4);
  both u5(a, b, y5);
  wide u6(a, b, y6);
  initial begin
    $monitor("%0t a%b b%b c%b va%b vb%b | %b %b %b%b %b %b %b", $time, a, b, c, va, vb, y1, y2, y3, z3, y4, y5, y6);
    a = 0; b = 0; c = 0; va = 0; vb = 0;
    #100 a = 1;
    #100 a = 0;
    #100 b = 1;
    #100 b = 0;
    #100 c = 1;
    #100 a = 1;
    #100 b = 1;
    #100 b = 0;
    #100 a = 0;
    #100 c = 0;
    #100 va = 1;
    #100 va = 3;
    #100 va = 2;
    #100 va = 0;
    #100 vb = 1;
    #100 vb = 3;
    #100 vb = 2;
    #100 vb = 0;
    #100 $finish;
  end
endmodule
