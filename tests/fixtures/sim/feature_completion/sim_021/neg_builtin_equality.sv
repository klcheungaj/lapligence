// IEEE 1800-2009 11.11: "equality and inequality between floats cannot be
// overloaded" -- unpacked records of one type already compare with ==.
module tb;
  typedef struct { int v; } t_t;
  function automatic bit ne(t_t a, t_t b);
    return 0;
  endfunction
  bind != function bit ne(t_t, t_t);
  t_t x, y;
  initial $display("%0d", x != y);
endmodule
