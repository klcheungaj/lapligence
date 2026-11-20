// A row of a multidimensional array as an `inside` item in a constraint: a
// located limit (docs/known_issues.md, constrained randomization limits).
module tb;
  class c;
    int m[2][3] = '{'{1, 2, 3}, '{4, 5, 6}};
    rand int x;
    constraint k { x inside {m[1]}; }
  endclass
  initial begin
    c h = new;
    $display("%0d", h.randomize());
    $display("%0d", h.x inside {4, 5, 6});
    $finish(0);
  end
endmodule
