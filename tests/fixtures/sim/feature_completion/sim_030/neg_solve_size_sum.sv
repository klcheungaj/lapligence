// A variable solved before an array size (S30-D19) is fixed before any
// element exists, so a constraint that ties it to the elements is circular:
// the size depends on the variable and the variable on the elements.
class c_c;
  rand bit [3:0] a [];
  rand bit [5:0] x;
  constraint c { a.size() inside {[1:3]}; x == a.sum(); solve x before a.size(); }
endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
