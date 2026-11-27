// SIM-004 negative: an output real port connected to an element of an
// associative array. IEEE 1800-2009 23.3.3.2 (SystemVerilog-1800-2009.txt
// L41483-41484) and 6.21 L7007-7008, as in neg_output_string_queue_element.
`timescale 1ns/1ns
module child (output real ro);
  initial #1 ro = 2.5;
endmodule
module tb;
  real aa [string];
  child c (.ro(aa["k"]));
  initial #2 $finish(0);
endmodule
