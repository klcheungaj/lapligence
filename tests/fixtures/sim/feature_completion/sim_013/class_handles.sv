// SIM-013 A01: event controls and a level wait on class properties. The
// waiter observes the property of the object its handle names: mutating that
// property wakes it, a sibling property or another object does not, and
// rebinding the handle (variable, array element or handle property) moves
// the observation to the new object (SV 9.4.2 `@(p.status)` example, 8.4).
// A nonvirtual method in an event expression follows the receiver's
// properties it reads (SV 9.4.2).
`timescale 1ns / 1ns
module tb;
  class Node;
    int v;
    int w;
    Node next;
    function int sum();
      return v + w;
    endfunction
  endclass

  Node h = new, h2 = new, old;
  Node hs[2];
  int i = 0;
  Node n;
  Node p;
  string l_v = "", l_h = "", l_sum = "", l_hs = "", l_n = "", l_pos = "";
  int t_wait = -1;

  initial begin
    h2.v = 7;
    old = h;
    hs[0] = new;
    hs[1] = new;
    hs[1].v = 4;
    n = new;
    n.next = new;
  end

  initial #0 forever begin @(h.v); l_v = {l_v, $sformatf(" %0t", $time)}; end
  initial #0 forever begin @(h); l_h = {l_h, $sformatf(" %0t", $time)}; end
  initial #0 forever begin @(h.sum()); l_sum = {l_sum, $sformatf(" %0t", $time)}; end
  initial #0 forever begin @(hs[i].v); l_hs = {l_hs, $sformatf(" %0t", $time)}; end
  initial #0 forever begin @(n.next.v); l_n = {l_n, $sformatf(" %0t", $time)}; end
  initial #0 forever begin @(posedge h.w[0]); l_pos = {l_pos, $sformatf(" %0t", $time)}; end
  // `p` is null until 19: the wait arms on the property without accessing it.
  initial begin
    #0 wait (p != null && p.v == 3);
    t_wait = $time;
  end

  initial begin
    #1 h.w = 1;
    #1 h.v = 2;
    #1 h = h2;
    #1 h2.v = 7;
    #1 h2.v = 8;
    #1 old.v = 9;
    #1 old.w = 1;
    #1 h = h2;
    #1 h2.w = 3;
    #1 hs[1].v = 5;
    #1 hs[0].v = 1;
    #1 i = 1;
    #1 hs[0].v = 2;
    #1 hs[1].v = 6;
    #1 n.next.v = 1;
    #1 n.next = new;
    #1 n.next.v = 0;
    #1 n.next.v = 2;
    #1 p = new;
    #1 p.v = 3;
    #1 hs[1] = hs[0];
    #1;
    $display("h.v:%s", l_v);
    $display("h:%s", l_h);
    $display("h.sum():%s", l_sum);
    $display("hs[i].v:%s", l_hs);
    $display("n.next.v:%s", l_n);
    $display("posedge h.w[0]:%s", l_pos);
    $display("wait: %0d", t_wait);
    $finish(0);
  end
endmodule
