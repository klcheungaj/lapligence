// IEEE 1800-2009 11.11: `+` is already legal for two ints and its int result
// is the prototype's result, so this declaration cannot overload it.
module tb;
  function automatic int f(int a, int b);
    return 999;
  endfunction
  bind + function int f(int, int);
  int x;
  initial x = 1 + 2;
endmodule
