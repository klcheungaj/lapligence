// SV2009 13.4: functions shall not contain time-controlled statements.
module tb;
  function automatic int f(input int x); #1; return x; endfunction
  initial $display("%0d", f(1));
endmodule
