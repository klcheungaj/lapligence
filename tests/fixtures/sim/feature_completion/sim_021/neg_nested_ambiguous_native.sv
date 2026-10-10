// IEEE 1800-2009 11.11: the inner sum could be either result type, so without
// a cast the nested overload is ambiguous.
module tb;
  typedef struct { int v; string n; } a_t;
  typedef struct { int v; string n; } b_t;
  function automatic a_t fa(a_t x, a_t y);
    fa = x;
  endfunction
  function automatic b_t fb(a_t x, a_t y);
    fb.v = x.v;
  endfunction
  function automatic a_t fba(b_t x, a_t y);
    fba = y;
  endfunction
  bind + function a_t fa(a_t, a_t);
  bind + function b_t fb(a_t, a_t);
  bind + function a_t fba(b_t, a_t);
  a_t x, y, z;
  initial z = (x + y) + x;
endmodule
