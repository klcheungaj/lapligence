// A slice of a packed array of structures is an array, not a structure
// (IEEE 1800-2009 7.4.5): it has no members.
module tb;
  typedef struct packed { logic [3:0] hi; logic [3:0] lo; } pair_t;
  pair_t [3:0] ps;
  initial $display("%h", ps[2:1].hi);
endmodule
