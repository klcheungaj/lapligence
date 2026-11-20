// rand_mode of one element of an array of packed values: a located limit
// (docs/known_issues.md, constrained randomization limits).
module tb;
  class c;
    rand bit [3:0] v[3];
  endclass
  initial begin
    c h = new;
    h.v[1].rand_mode(0);
    $display("%0d", h.randomize());
    $finish(0);
  end
endmodule
