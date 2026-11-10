// Reading the mode of one element of a queue of class handles: the elements
// have no mode storage yet, so an explicit error, never the array's mode.
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
    $display("%0d", n.q[0].rand_mode());
    $finish;
  end
endmodule
