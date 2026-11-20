// A solve...before side naming an array element at a non-constant index: a
// located limit (docs/known_issues.md, constrained randomization limits).
module tb;
  class c;
    rand int a[4];
    rand int x;
    int i = 1;
    constraint k { a[i] < x; x < 10; x > 0; solve a[i] before x; }
  endclass
  initial begin
    c h = new;
    $display("%0d", h.randomize());
    $finish(0);
  end
endmodule
