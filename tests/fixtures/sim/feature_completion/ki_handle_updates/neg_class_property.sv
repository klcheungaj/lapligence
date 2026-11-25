// A property of an object is a member of a dynamic variable (readme:
// "Rejected").
module tb;
  class node;
    node next;
  endclass
  node h = new, n = new;
  initial begin
    h.next <= n;
  end
endmodule
