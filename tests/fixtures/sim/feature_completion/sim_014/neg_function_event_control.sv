// SIM-014 negative: a function shall not contain timing controls
// (SV 13.4), including an intra-assignment event control.
module tb;
  event e;
  function automatic int f();
    int x;
    x = @e 1;
    return x;
  endfunction
  int y;
  initial y = f();
endmodule
