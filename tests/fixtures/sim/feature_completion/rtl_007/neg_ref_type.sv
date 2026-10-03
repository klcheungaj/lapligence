// SV2009 13.5.2: a ref actual must have a type equivalent to the formal.
module tb;
  typedef struct { logic [7:0] a; } r1_t;
  typedef struct { logic [7:0] a; } r2_t;
  function automatic void f(ref r1_t x); x.a = 0; endfunction
  r2_t r;
  initial f(r);
endmodule
