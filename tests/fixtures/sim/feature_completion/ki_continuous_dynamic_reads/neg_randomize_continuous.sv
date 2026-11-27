// A randomize() call used as a value in a continuous assignment: a method of
// the class object a handle names, which 6.21 limits to procedural blocks.
module tb;
  class c;
    rand bit [3:0] x;
  endclass
  c h = new;
  wire w;
  assign w = h.randomize();
  initial #1 $display("%b", w);
endmodule
