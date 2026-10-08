// SIM-018: a long loop creates two-object cycles and drops them every time
// step; the collector keeps the live object count bounded (SV 8.27). One
// object stays reachable from a module variable for the whole run.
class node_c;
  int v;
  node_c peer;
  int pad[4];
  function new(int init);
    v = init;
  endfunction
endclass

module tb;
  node_c keep;
  int sum;

  initial begin
    keep = new(-1);
    for (int i = 0; i < 20000; i++) begin
      node_c a, b;
      a = new(i);
      b = new(i + 1);
      a.peer = b;
      b.peer = a;
      a.pad[3] = i;
      sum += a.peer.v - b.peer.v + b.peer.pad[3] - i;
      #1;
    end
    $display("sum=%0d keep=%0d", sum, keep.v);
    $finish(0);
  end
endmodule
