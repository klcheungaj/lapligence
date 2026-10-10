// IEEE 1800-2009 16.6: functions in concurrent assertion expressions "shall
// be automatic (or preserve no state information) and have no side effects."
// The overload's bound function counts its calls; the assertion is rejected
// (llg does not admit function calls in concurrent assertion expressions).
module tb;
  typedef struct { int v; } a_t;
  int calls;
  function automatic bit flt(a_t x, a_t y);
    calls++;
    return x.v < y.v;
  endfunction
  bind < function bit flt(a_t, a_t);
  a_t x, y;
  logic clk = 0;
  always #5 clk = ~clk;
  a1: assert property (@(posedge clk) x < y);
  initial #20 $finish;
endmodule
