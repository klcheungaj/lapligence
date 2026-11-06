// An element of a handle array needs a constant, state or loop index (18.4);
// which object a random index would name is not known before solving.
class leaf_c; rand bit [3:0] v; endclass
class c_c;
  rand leaf_c l [2];
  rand bit k;
  constraint c { l[k].v == 1; }
endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish(0);
  end
endmodule
