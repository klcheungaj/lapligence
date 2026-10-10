// SV 10.6.2: a member of a packed structure variable is a part-select of a
// variable, not a singular variable.
module tb;
  typedef struct packed {
    logic [3:0] hi;
    logic [3:0] lo;
  } ps_t;
  ps_t s;
  initial force s.hi = 4'hf;
endmodule
