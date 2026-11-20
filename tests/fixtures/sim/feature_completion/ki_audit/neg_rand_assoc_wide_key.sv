// A random associative array with an index type wider than 64 bits: a
// located limit (docs/known_issues.md, constrained randomization limits).
module tb;
  class c;
    rand int m[bit [79:0]];
    constraint k { foreach (m[i]) m[i] < 5; }
  endclass
  initial begin
    c h = new;
    h.m[1] = 0;
    $display("%0d", h.randomize());
    $finish(0);
  end
endmodule
