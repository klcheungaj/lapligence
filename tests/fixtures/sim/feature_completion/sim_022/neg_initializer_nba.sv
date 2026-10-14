// SIM-022-A03 nearest illegal: a static local's initializer reaches, through
// a second function, a nonblocking assignment that would take effect after
// the call returns.
module tb;
  int flag;
  function automatic void mark();
    flag <= 1;
  endfunction
  function automatic int helper();
    mark();
    return 2;
  endfunction
  function automatic int cached();
    static int value = helper();
    return value;
  endfunction
  initial $display("cached=%0d flag=%0d", cached(), flag);
endmodule
