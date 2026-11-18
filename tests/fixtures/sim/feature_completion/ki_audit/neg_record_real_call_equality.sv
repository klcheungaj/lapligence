// A record call result with a real member as an equality operand: a located
// limit (docs/known_issues.md, record results with real members as equality
// operands).
module tb;
  typedef struct { int i; real r; } rec_t;
  rec_t a = '{1, 2.5};
  function automatic rec_t mk(real r);
    mk.i = 1;
    mk.r = r;
  endfunction
  initial begin
    $display("%0d", mk(2.5) == a);
    $finish;
  end
endmodule
