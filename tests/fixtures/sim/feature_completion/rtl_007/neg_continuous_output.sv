// SV2009 13.4: functions with output/inout/ref formals are illegal in continuous assignments.
module tb;
  typedef logic [7:0] arr_t [0:1];
  arr_t a; logic [7:0] y;
  function automatic logic [7:0] f(output arr_t o); o = '{1,2}; return 3; endfunction
  assign y = f(a);
endmodule
