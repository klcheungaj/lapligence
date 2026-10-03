// SV2009 11.4.2: a function call result is not an assignable operand.
module tb;
  int x, y;
  function automatic int f(int a);
    return a;
  endfunction
  initial y = f(x)++;
endmodule
