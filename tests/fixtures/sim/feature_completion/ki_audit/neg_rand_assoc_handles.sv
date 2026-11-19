// A random associative array of class handles: a located limit
// (docs/known_issues.md, constrained randomization limits).
module tb;
  class item; rand int v; endclass
  class c;
    rand item m[int];
    constraint k { foreach (m[i]) m[i].v < 5; }
  endclass
  initial begin
    c h = new;
    h.m[0] = new;
    $display("%0d", h.randomize());
    $finish(0);
  end
endmodule
