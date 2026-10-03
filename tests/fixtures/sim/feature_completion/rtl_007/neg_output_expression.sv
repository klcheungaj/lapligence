// SV2009 13.5.1: an output actual must be an assignable variable, not an expression.
module tb;
  typedef logic [7:0] arr_t [0:3];
  function automatic void f(output arr_t x); x = '{1,2,3,4}; endfunction
  arr_t a, b;
  initial f(a == b ? a : b);
endmodule
