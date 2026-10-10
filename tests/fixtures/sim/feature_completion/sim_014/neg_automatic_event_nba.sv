// SIM-014 A03 negative: an automatic variable cannot be the target of a
// nonblocking assignment, including one with an intra-assignment repeated
// event control (SV 6.21, 10.4.2, 13.3.2).
module tb;
  event e;
  task automatic t();
    int x;
    x <= repeat (2) @e 1;
  endtask
  initial t();
endmodule
