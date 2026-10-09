// SIM-024: `%p` of class handles: null, the dynamic class's properties
// (base class first), cycles, shared objects and the nesting bound
// (SV 21.2.1.7; handle presentation is implementation dependent and is
// documented in the readme).
module tb;
  class Node;
    int v;
    Node next;
    function new(int v);
      this.v = v;
    endfunction
  endclass
  class Leaf extends Node;
    string label;
    real w [$];
    function new(int v, string label);
      super.new(v);
      this.label = label;
    endfunction
  endclass
  class Holder;
    Node items [$];
    chandle h;
  endclass
  Node n0, n1, n2, self_loop, chain;
  Leaf lf;
  Holder ho;
  Node base_view;
  initial begin
    $display("A|%p|%0p|", n0, n0);
    n1 = new(1);
    n2 = new(2);
    $display("B|%p|", n1);
    n1.next = n2;
    n2.next = n1;
    $display("C|%p|", n1);
    $display("D|%0p|", n2);
    self_loop = new(7);
    self_loop.next = self_loop;
    $display("E|%p|", self_loop);
    lf = new(3, "leaf");
    lf.w.push_back(0.5);
    lf.next = n1;
    base_view = lf;
    $display("F|%p|", base_view);
    ho = new;
    ho.items.push_back(n2);
    ho.items.push_back(n2);
    ho.items.push_back(null);
    $display("G|%p|", ho);
    for (int k = 0; k < 70; k++) begin
      Node fresh;
      fresh = new(k);
      fresh.next = chain;
      chain = fresh;
    end
    $display("H|%0p|", chain);
    $display("I|%0d|%0d|%0d|%s|", n1.v, n1.next.v, ho.items.size(), lf.label);
    $finish(0);
  end
endmodule
