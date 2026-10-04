// SIM-003 boundary: a conditional whose operands are native records is legal
// (IEEE 1800-2009 11.4.11), but an unknown predicate needs a per-member merge
// that native leaves have no fixed payload for, so llg rejects it explicitly.
module tb;
  typedef struct {string s; int n;} T;
  T a, b, r;
  logic sel;
  function automatic T pick(input logic c, input T x, y);
    return c ? x : y;
  endfunction
  initial begin
    a.s = "a";
    b.s = "b";
    r = pick(sel, a, b);
    $display("%s", r.s);
    $finish(0);
  end
endmodule
