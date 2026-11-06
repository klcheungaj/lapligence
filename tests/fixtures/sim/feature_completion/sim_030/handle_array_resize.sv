// The size of a dynamic array or queue of handles is solved with the other
// sizes (18.4, S30-D5). Randomize allocates nothing: elements up to the new size
// keep their objects and are randomized; a new element is null, which a guard
// absorbs (S30-D13). post_randomize runs only on objects still reached.
class leaf_c;
  rand bit [3:0] v;
  int id;
  constraint lc { v < 8; }
  function new(int i);
    id = i;
  endfunction
  function void post_randomize();
    $display("post %0d", id);
  endfunction
endclass

class c_c;
  rand leaf_c a[];
  rand leaf_c q[$];
  rand int unsigned n;
  constraint s {
    a.size() == n;
    q.size() == n;
    n inside {[1:5]};
  }
  constraint e {
    foreach (a[i]) a[i] != null -> a[i].v == 7;
    foreach (q[i]) q[i] != null -> q[i].v == 6;
  }
endclass

module tb;
  initial begin
    static c_c c = new;
    int ok;
    leaf_c first;
    leaf_c second;
    leaf_c third;
    c.a = new[2];
    first = new(0);
    second = new(1);
    third = new(2);
    c.a[0] = first;
    c.a[1] = second;
    c.q.push_back(third);
    ok = c.randomize() with { n == 1; };
    $display("%0d a=%0d q=%0d v=%0d %0d", ok, c.a.size(), c.q.size(), first.v, third.v);
    $display("same %0d", c.a[0] == first);
    ok = c.randomize() with { n == 3; };
    $display("%0d a=%0d q=%0d", ok, c.a.size(), c.q.size());
    $display("null %0d %0d %0d %0d", c.a[1] == null, c.a[2] == null, c.q[1] == null, c.q[2] == null);
    $finish(0);
  end
endmodule
