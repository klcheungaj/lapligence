// SV2009 6.21, 10.5, 13.4: a declaration initializer may call only zero-time
// functions; a function cannot contain a delay.
module tb;
  function int slow();
    #1;
    return 7;
  endfunction
  int x = slow();
  initial $finish;
endmodule
