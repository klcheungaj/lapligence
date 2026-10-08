// SIM-018: a running timed method keeps its receiver: once the only handle
// variable is cleared, `this` of the suspended activation still reaches the
// object and its cycle (SV 8.27, 8.10, 13.5).
`timescale 1ns/1ns
class node_c;
  int v;
  node_c peer;
  function new(int init);
    v = init;
  endfunction
  task automatic slow_sum(output int r);
    #30;
    r = v + peer.v + peer.peer.v;
  endtask
endclass

module tb;
  node_c h;
  int r;

  initial begin
    h = new(3);
    h.peer = new(4);
    h.peer.peer = h;
    fork
      h.slow_sum(r);
      #1 h = null;
      for (int i = 0; i < 20; i++) begin
        node_c g;
        g = new(i);
        g.peer = g;
        #1;
      end
    join
    $display("r=%0d null=%0d", r, h == null);
    $finish(0);
  end
endmodule
