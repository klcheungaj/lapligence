// SIM-003 A03: a chandle is not an integral type, so it cannot be a packed
// structure member. IEEE 1800-2009 6.14, 7.2.1.
module tb;
  typedef struct packed {chandle h; logic [7:0] n;} bad_t;
  bad_t v;
  initial $finish(0);
endmodule
