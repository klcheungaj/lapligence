// SV2009 13.5.2: a const ref actual cannot be a conditional temporary.
module tb;
  typedef logic [7:0] arr_t [0:1];
  function automatic int f(const ref arr_t x); return x[0]; endfunction
  arr_t a, b;
  initial $display("%0d", f(a == b ? a : b));
endmodule
