// The same through a second order: y is solved before x, x before the size,
// and a foreach compares the elements with y.
class c_c;
  rand bit [3:0] a [$];
  rand bit [3:0] x, y;
  constraint c { a.size() inside {[1:3]}; y <= x; x < 4; }
  constraint order { solve y before x; solve x before a.size(); }
  constraint elems { foreach (a[i]) a[i] > y; }
endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
