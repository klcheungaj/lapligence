// The mode of one element of a dynamic array or queue of class handles has no
// storage yet: an explicit error, never the object form.
class leaf;
  rand int x;
endclass
class node;
  rand leaf q[$];
endclass
module tb;
  node n = new;
  initial begin
    leaf l = new;
    n.q.push_back(l);
    n.q[0].rand_mode(0);
    $finish;
  end
endmodule
