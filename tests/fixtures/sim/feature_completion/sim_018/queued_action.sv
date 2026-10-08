// SIM-018: queued actions keep what they refer to. A delayed nonblocking
// assignment holds the only reference to a cycle until it commits, and a
// zero-delay one holds a cycle across the Active-region turns that run
// before the NBA region (SV 8.27, 4.4.2.4, 10.4.2).
`timescale 1ns/1ns
class node_c;
  int v;
  node_c peer;
  function new(int init);
    v = init;
  endfunction
endclass

module tb;
  node_c later;
  node_c soon;

  function automatic node_c make_cycle(int base);
    node_c a, b;
    a = new(base);
    b = new(base + 1);
    a.peer = b;
    b.peer = a;
    return a;
  endfunction

  initial later <= #20 make_cycle(7);
  initial #3 soon <= make_cycle(30);

  initial begin
    for (int i = 0; i < 30; i++) begin
      node_c g;
      g = new(i);
      g.peer = g;
      #1;
    end
    $display("later %0d %0d %0d soon %0d %0d %0d", later.v, later.peer.v,
             later.peer.peer.v, soon.v, soon.peer.v, soon.peer.peer.v);
    $finish(0);
  end
endmodule
