// Constrained randomization limits: a randomize() call used as a value
// outside procedural code.
module tb;
  class c;
    rand bit [3:0] x;
  endclass
  c h = new;
  wire w;
  assign w = h.randomize();
  initial #1 $display("%b", w);
endmodule
