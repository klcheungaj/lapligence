// SV2009 13.4: functions with ref formals are illegal in event expressions.
module tb;
  typedef logic [7:0] arr_t [0:1];
  arr_t a;
  function automatic logic [7:0] f(ref arr_t o); o[0] = 1; return 3; endfunction
  initial @(f(a)) $display("x");
endmodule
