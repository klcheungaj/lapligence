// Sizes are solved before everything else (S30-D5), so no variable can be
// solved before one.
class c_c;
  rand bit [3:0] a [];
  rand bit [3:0] n;
  constraint c { a.size() == n; solve n before a.size(); }
endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
