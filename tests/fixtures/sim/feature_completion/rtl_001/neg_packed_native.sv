// SV2009 section 7.2.1: packed structure members must be integral.
module tb;
  typedef struct packed { logic [7:0] value; string text; } illegal_t;
  illegal_t value;
endmodule
