// SIM-014 A03 negative: SV 6.21 forbids nonblocking writes to class
// properties (members of dynamic objects), with or without timing control.
module tb;
  class Box;
    int x;
  endclass
  event e;
  Box h;
  initial h = new;
  initial #1 h.x <= @e 5;
endmodule
