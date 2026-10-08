// SIM-018: a cycle stays alive while only suspended processes refer to it:
// a join_none branch that captured the creating task's handle variable and
// keeps a local of its own, after the task has returned (SV 8.27, 9.3.2).
`timescale 1ns/1ns
class node_c;
  int v;
  node_c peer;
  function new(int init);
    v = init;
  endfunction
endclass

module tb;
  task automatic hold_cycle(int base);
    node_c a, b;
    a = new(base);
    b = new(base + 1);
    a.peer = b;
    b.peer = a;
    fork
      begin
        node_c c;
        c = a.peer;
        #50;
        $display("child %0d %0d %0d %0d", a.v, c.v, c.peer.v, c.peer.peer.v);
      end
    join_none
  endtask

  task automatic churn(int n);
    for (int i = 0; i < n; i++) begin
      node_c g;
      g = new(i);
      g.peer = g;
      #1;
    end
  endtask

  initial begin
    hold_cycle(10);
    churn(40);
    $display("parent done at %0d", $time);
    #20 $finish(0);
  end
endmodule
