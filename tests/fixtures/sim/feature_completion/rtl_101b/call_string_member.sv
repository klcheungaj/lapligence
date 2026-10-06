// RTL-101b with SIM-007: scalar members of a column-layout record call
// result, including a string member, are read inside an expression; each
// call runs once into a temporary.
module tb;
  typedef struct { logic [1023:0] w [0:2047]; string n; int k; } big_t;
  big_t r;
  int calls;
  function automatic big_t f(input big_t x, string s);
    calls++;
    x.n = {x.n, s};
    x.k = calls;
    return x;
  endfunction
  initial begin
    r.n = "z";
    $display("%s %0d %0d", f(r, "!").n, f(r, "?").k, calls);
    $finish(0);
  end
endmodule
