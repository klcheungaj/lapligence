// SV2009 13.4: functions shall not enable tasks.
module tb;
  task automatic t(); #1; endtask
  function automatic int f(input int x); t(); return x; endfunction
  initial $display("%0d", f(1));
endmodule
