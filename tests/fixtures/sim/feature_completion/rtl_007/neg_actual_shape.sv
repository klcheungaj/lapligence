// SV2009 13.5: an unpacked actual must be assignment compatible with the formal.
module tb;
  typedef logic [7:0] arr_t [0:3];
  typedef logic [7:0] arr3_t [0:2];
  function automatic int f(input arr_t x); return x[0]; endfunction
  arr3_t b;
  initial $display("%0d", f(b));
endmodule
