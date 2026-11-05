// A run-time selected record element as a `ref` actual of a function called in
// an expression is legal (SV 13.5.2); only statement calls dispatch on the index.
module tb;
  typedef struct { string s; int n; } el_t;
  typedef struct { el_t e[2]; } rec_t;
  rec_t o;
  int i = 1;
  int x;
  function automatic int f(ref el_t r);
    return r.n;
  endfunction
  initial x = f(o.e[i]);
endmodule
