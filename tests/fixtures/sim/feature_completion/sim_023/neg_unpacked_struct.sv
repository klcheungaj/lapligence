// SV 10.6.2, 6.4: an unpacked structure variable is not singular.
module tb;
  typedef struct {
    logic [3:0] a;
    logic [3:0] b;
  } us_t;
  us_t us;
  initial release us;
endmodule
