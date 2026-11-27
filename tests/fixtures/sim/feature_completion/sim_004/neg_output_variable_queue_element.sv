// SIM-004 negative: an output port connected to an element of a queue that a
// variable selects. IEEE 1800-2009 23.3.3.2 (SystemVerilog-1800-2009.txt
// L41483-41484) and 6.21 L7007-7008, as in neg_output_string_queue_element.
`timescale 1ns/1ns
module child (output logic [7:0] po);
  initial #1 po = 8'h5a;
endmodule
module tb;
  logic [7:0] qv [$] = '{0, 0};
  int i = 1;
  child c (.po(qv[i]));
  initial #2 $finish(0);
endmodule
