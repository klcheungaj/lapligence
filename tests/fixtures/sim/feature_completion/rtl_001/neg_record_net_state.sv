// SV2009 section 6.7: each unpacked net member must be a legal net type.
module tb;
  typedef struct { logic [7:0] value; bit flag; } illegal_t;
  wire illegal_t value;
endmodule
