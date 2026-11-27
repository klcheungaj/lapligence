// SIM-004 negative: an output string port connected to an element of a
// queue in a design whose pragma lowers the read rule of 6.21 to a warning.
// The connection still writes the element with a continuous assignment,
// which 6.21 forbids on its own. IEEE 1800-2009 23.3.3.2
// (SystemVerilog-1800-2009.txt L41483-41484) and 6.21 L7007-7008, as in
// neg_output_string_queue_element (decision A19-D95).
`pragma diagnostic ignore="-Wdynamic-non-procedural"
`timescale 1ns/1ns
module child (output string so);
  initial #1 so = "out";
endmodule
module tb;
  string q [$] = '{"q0", "q1"};
  child c (.so(q[1]));
  initial #2 $finish(0);
endmodule
