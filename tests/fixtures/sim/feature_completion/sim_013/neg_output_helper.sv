// SIM-013 A03 negative: a function with output, inout or ref formals cannot
// be called in an event expression (SV 13.4).
module tb;
  int a = 0, o;
  function automatic int f(input int x, output int y);
    y = x;
    return x;
  endfunction
  initial begin
    @(f(a, o));
    $display("%0d", o);
  end
  initial #1 a = 1;
endmodule
