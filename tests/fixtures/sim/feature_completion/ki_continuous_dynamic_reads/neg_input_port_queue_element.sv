// An input port connected to an element of a queue that a variable selects (6.21, 23.3.3.2).
`timescale 1ns/1ps
module child (input string s);
  initial #1 $display("s=%s", s);
endmodule
module tb;
  string sq [$] = '{"a", "b"};
  int i = 1;
  child c (.s(sq[i]));
endmodule
