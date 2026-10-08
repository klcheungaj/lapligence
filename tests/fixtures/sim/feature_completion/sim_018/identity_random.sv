// SIM-018: collection is unobservable. Handle equality keeps comparing
// identities while garbage is reclaimed around it, and the process random
// stream and $urandom draws do not depend on whether or when the collector
// runs (SV 8.27, 8.4, 18.13, 18.14).
class node_c;
  int v;
  node_c peer;
  function new(int init);
    v = init;
  endfunction
endclass

module tb;
  node_c a, b, c;
  int unsigned draws[8];

  initial begin
    process::self().srandom(5);
    a = new(1);
    b = a;
    c = new(1);
    for (int i = 0; i < 8; i++) begin
      node_c g;
      g = new(i);
      g.peer = g;
      draws[i] = $urandom;
      #1;
    end
    $display("eq %0d %0d %0d %0d", a == b, a == c, a != c, b.v == c.v);
    b = new(2);
    $display("eq %0d %0d %0d", a == b, b == c, a.peer == null);
    for (int i = 0; i < 8; i++) $display("draw %0d %0d", i, draws[i]);
    $finish(0);
  end
endmodule
