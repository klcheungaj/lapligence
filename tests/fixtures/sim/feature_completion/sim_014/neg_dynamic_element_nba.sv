// SIM-014 A03 negative: elements of dynamically sized variables shall not
// be written with nonblocking assignments (SV 6.21), so a queue element
// cannot be the target of an event-controlled NBA.
module tb;
  event e;
  int q[$];
  initial q = {0, 0};
  initial #1 q[1] <= @e 5;
endmodule
