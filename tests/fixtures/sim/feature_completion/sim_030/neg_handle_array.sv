// Arrays of rand class handles are not supported.
class leaf_c; rand bit [3:0] v; endclass
class c_c;
  rand leaf_c l [2];
  constraint c { foreach (l[i]) l[i].v == 1; }
endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
