// IEEE 1800-2009 section 6.7 permits four-state integral types, fixed arrays
// and unpacked structures of valid net types; an unpacked union is excluded.
module tb;
  typedef union { logic [7:0] byte_value; logic [3:0] nibble; } union_t;
  typedef struct { union_t member; } record_t;
  wire record_t values[1:0];
endmodule
