// SIM-004 negative: an output port of a packed type connected to a bit of an
// element of a dynamic array, inside a concatenation. IEEE 1800-2009
// 23.3.3.2 (SystemVerilog-1800-2009.txt L41483-41484) and 6.21 L7007-7008,
// as in neg_output_string_queue_element.
`timescale 1ns/1ns
module child (output logic [1:0] po);
  initial #1 po = 2'b10;
endmodule
module tb;
  logic [7:0] d [];
  logic x;
  child c (.po({x, d[1][0]}));
  initial #2 $finish(0);
endmodule
