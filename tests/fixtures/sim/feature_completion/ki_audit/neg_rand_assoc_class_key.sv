// A random associative array with a class index type: a located limit
// (docs/known_issues.md, constrained randomization limits).
module tb;
  class key; endclass
  class c;
    rand int m[key];
    constraint k { foreach (m[i]) m[i] < 5; }
  endclass
  initial begin
    c h = new;
    key kk = new;
    h.m[kk] = 0;
    $display("%0d", h.randomize());
    $finish(0);
  end
endmodule
