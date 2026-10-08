// SIM-018: cycles whose only references are a queued mailbox message and a
// message already handed to a blocked getter that has not resumed yet stay
// alive until they are received (SV 8.27, 15.4).
`timescale 1ns/1ns
class node_c;
  int v;
  node_c peer;
  function new(int init);
    v = init;
  endfunction
endclass

module tb;
  mailbox #(node_c) queued = new();
  mailbox #(node_c) handed = new();

  function automatic node_c make_cycle(int base);
    node_c a, b;
    a = new(base);
    b = new(base + 1);
    a.peer = b;
    b.peer = a;
    return a;
  endfunction

  task automatic churn(int n);
    for (int i = 0; i < n; i++) begin
      node_c g;
      g = new(i);
      g.peer = g;
      #1;
    end
  endtask

  initial begin
    node_c got;
    handed.get(got);
    $display("handed %0d %0d %0d at %0d", got.v, got.peer.v, got.peer.peer.v, $time);
  end

  initial begin
    queued.put(make_cycle(1));
    churn(30);
    begin
      node_c x;
      queued.get(x);
      $display("queued %0d %0d %0d n=%0d", x.v, x.peer.v, x.peer.peer.v, queued.num());
    end
    handed.put(make_cycle(5));
    churn(3);
    $finish(0);
  end
endmodule
