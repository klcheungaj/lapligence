// SIM-004 negative: an output string port connected to an element of a
// queue. IEEE 1800-2009 23.3.3.2 (SystemVerilog-1800-2009.txt
// L41483-41484): "A continuous assignment shall be implied when a variable is
// connected to the output port of an instance." 6.21 L7007-7008: "Automatic
// variables and members or elements of dynamic variables—class properties
// and dynamically sized variables—shall not be written with nonblocking,
// continuous, or procedural continuous assignments." Some simulators accept
// it; llg follows the text (decision A19-D95).
`timescale 1ns/1ns
module child (output string so);
  initial #1 so = "out";
endmodule
module tb;
  string q [$] = '{"q0", "q1"};
  child c (.so(q[1]));
  initial #2 $finish(0);
endmodule
