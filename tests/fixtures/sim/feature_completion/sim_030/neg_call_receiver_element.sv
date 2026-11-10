// A constraint function called on an element of a random handle array
// (`more[i].cap(x)`) is not served: the receiver would be read from the array
// the same call may resize. A scalar handle receiver and the owner's own
// functions work (linked_function_receivers).
class node;
  rand bit [3:0] v;
  bit [3:0] lim;
  rand node more[$];
  function bit [3:0] cap(bit [3:0] x);
    return x < lim ? x : lim;
  endfunction
  constraint tree { foreach (more[i]) more[i].v >= more[i].cap(4'd2); }
endclass
module tb;
  initial begin
    static node n = new;
    void'(n.randomize());
    $finish;
  end
endmodule
