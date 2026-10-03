// SV2009 13.5.2: a ref actual cannot be a function-call temporary.
module tb;
  typedef logic [7:0] arr_t [0:3];
  function automatic arr_t g(); return '{1,2,3,4}; endfunction
  function automatic void f(ref arr_t x); x[0] = 0; endfunction
  initial f(g());
endmodule
