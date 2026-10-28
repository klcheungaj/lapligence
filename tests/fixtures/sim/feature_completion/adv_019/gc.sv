// ADV-019 family 5, collection: the VPI application gc.c follows class
// objects from construction to reclamation (IEEE 1800-2009 37.3.7, 37.29,
// 38.36.1). Run with LLG_GC_STRESS=1 and LLG_GC_VERIFY=1: every safe point
// after an allocation collects, so an object no variable refers to is
// reclaimed before the next time step. See readme.md for the expected output.
module tb;
  class Node;
    int v;
    Node link;
    int items[$];
    function new(int x);
      v = x;
      items.push_back(x);
    endfunction
  endclass
  class Leaf extends Node;
    function new(int x);
      super.new(x);
    endfunction
  endclass
  Node keep;
  Node tmp;
  function automatic void make(int x);
    tmp = new(x);
    tmp.link = keep;
  endfunction
  function automatic void copy();
    tmp = new keep;
  endfunction
  function automatic void cycle();
    Node a = new(10);
    Node b = new(11);
    a.link = b;
    b.link = a;
  endfunction
  function automatic void leaf();
    Leaf l = new(20);
    l.link = keep;
  endfunction
  initial begin
    $setup();
    keep = new(1);
    make(2);
    $grab();
    copy();
    #1 $check(1);
    cycle();
    #1 $check(2);
    leaf();
    #1 $check(3);
    tmp = null;
    #1 $check(4);
    $display("keep.v=%0d tmp=%0d link=%0d", keep.v, tmp == null, keep.link == null);
  end
endmodule
