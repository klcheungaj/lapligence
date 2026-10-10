// IEEE 1800-2009 4.4.2.9, 11.11: $monitor arguments are evaluated in the
// read-only Postponed region; an overloaded operator whose bound function
// writes module storage is rejected there like the same function call.
module tb;
  typedef struct { int v; } a_t;
  int calls;
  function automatic bit flt(a_t x, a_t y);
    calls++;
    return x.v < y.v;
  endfunction
  bind < function bit flt(a_t, a_t);
  a_t x, y;
  initial $monitor("%0d", x < y);
endmodule
