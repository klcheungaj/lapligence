// Decision PX-D4: when an actual and a port differ in width, a whole port is
// a continuous assignment that widens or truncates the value, but a port
// expression (a part-select or a concatenation in the module header)
// connects bit by bit from the least significant bit: only the low bits that
// both sides have are connected. The port expression's higher bits stay
// unconnected when the actual is narrower (z on a net), and a wider actual's
// higher bits stay undriven (z on a net, the default x on a variable).
//
// IEEE 1800-2009 23.3.3 (SystemVerilog-1800-2009.txt L41452-41454):
//   "Each port connection shall be a continuous assignment of source to sink,
//   where one connected item shall be a signal source and the other shall be
//   a signal sink. The assignment shall be a continuous assignment from
//   source to sink for input or output ports."
// IEEE 1800-2009 23.2.2.1 (L40430-40434) lists the port references (a bit-
// select, a part-select, a concatenation) without a rule for an actual of a
// different width.
module whole_in(input [1:0] c);
endmodule

module part_in(.p(c[2:1]));
  input [3:0] c;
endmodule

module cat_in(.p({a, b}));
  input [1:0] a;
  input b;
endmodule

module whole_out(output [2:0] y);
  assign y = 3'b110;
endmodule

module part_out(.p(r[2:0]));
  output [3:0] r;
  assign r = 4'b0110;
endmodule

module cat_out(.p({a, b}));
  output [1:0] a;
  output b;
  assign a = 2'b11;
  assign b = 1'b0;
endmodule

module tb;
  wire one;
  assign one = 1'b1;
  wire [4:0] w1, w2, w3;
  logic [4:0] v3;
  whole_in u1(.c(one));
  part_in u2(.p(one));
  cat_in u3(.p(one));
  cat_in u4(.p(2'b11));
  whole_out u5(.y(w1));
  part_out u6(.p(w2));
  cat_out u7(.p(w3));
  cat_out u8(.p(v3));
  initial #1 begin
    $display("whole in c=%b", u1.c);
    $display("part in c=%b", u2.c);
    $display("cat in a=%b b=%b", u3.a, u3.b);
    $display("cat in 2 bits a=%b b=%b", u4.a, u4.b);
    $display("whole out w1=%b", w1);
    $display("part out w2=%b", w2);
    $display("cat out w3=%b v3=%b", w3, v3);
    $finish;
  end
endmodule
