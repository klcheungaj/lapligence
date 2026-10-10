// IEEE 1800-2009 11.11: "If no expected data type exists because the operator
// is in a self-determined context, then a cast shall be used".
module tb;
  typedef struct { int v; string n; } a_t;
  typedef struct { int v; string n; } b_t;
  function automatic a_t fa(a_t x, a_t y);
    fa = x;
  endfunction
  function automatic b_t fb(a_t x, a_t y);
    fb.v = x.v;
  endfunction
  bind + function a_t fa(a_t, a_t);
  bind + function b_t fb(a_t, a_t);
  a_t x, y;
  initial $display("%p", x + y);
endmodule
