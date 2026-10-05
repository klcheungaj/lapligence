// IEEE 1800-2009 11.11: only the comparison's own operand takes the other
// operand's type; a nested operand has no single expected type.
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
  s_t a, b, c;
  initial $display("%0d", (a + b + a) < c);
endmodule
