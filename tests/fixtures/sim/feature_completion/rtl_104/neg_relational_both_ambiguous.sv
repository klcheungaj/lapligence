// IEEE 1800-2009 11.11: when both operands of a comparison need the other's
// type, neither comparison is unambiguous and a cast is required.
typedef struct { int v; } s_t;
typedef struct { int w; } t_t;
function automatic s_t adds(s_t a, s_t b);
  adds.v = a.v + b.v;
endfunction
function automatic t_t addt(s_t a, s_t b);
  addt.w = a.v + b.v;
endfunction
function automatic bit lts(s_t a, s_t b);
  return a.v < b.v;
endfunction

module tb;
  bind + function s_t adds(s_t, s_t);
  bind + function t_t addt(s_t, s_t);
  bind < function bit lts(s_t, s_t);
  s_t a, b;
  initial $display("%0d", (a + b) < (b + a));
endmodule
