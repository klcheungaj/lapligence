// A random array read whole (here compared with another array) would be
// state to the solver: a located error.
class c_c;
  rand bit [3:0] a [4];
  bit [3:0] b [4];
  constraint c { a == b; }
endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
