// A randomize() call in an event control: a located limit
// (docs/known_issues.md, constrained randomization limits).
module tb;
  class c; rand int v; endclass
  c h = new;
  initial begin
    fork
      @(h.randomize());
    join_none
    #1 $finish(0);
  end
endmodule
