// A net lvalue takes only constant selects (IEEE 1800-2009 A.8.5).
module tb;
  typedef struct packed { logic [3:0] hi; logic [3:0] lo; } pair_t;
  wire pair_t [1:0] wn;
  integer i;
  assign wn[i].lo = 4'h1;
endmodule
