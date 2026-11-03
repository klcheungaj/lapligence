// SIM-014 / FIN-001 D1: a detached repeated-event NBA commits only after its
// n-th event (SV 9.4.5). Values are read between events.
module tb;
  class C;
    int v;
    bit w;
    function new(int x);
      v = x;
    endfunction
  endclass
  C h, k = new(7);
  C g = new(0);
  string t = "a";
  real r = 0.0;
  int x = 0, y = 0, z = 0;
  bit clk = 1'b0;
  event e, f;
  int fired = 0;
  always @(f) fired++;
  initial begin
    t <= repeat (3) @(e) "rep";
    r <= repeat (3) @(e) 1.5;
    h <= repeat (3) @(e) k;
    x <= repeat (3) @(posedge g.w) 5;
    y <= repeat (3) @(posedge clk) 6;
    z <= repeat (0) @(e) 9;
    ->> repeat (2) @(posedge g.w) f;
  end
  initial begin
    #1 $display("t1 %s %0.1f %0d %0d %0d %0d %0d", t, r, h == null, x, y, z, fired);
    -> e;
    g.w = 1'b1;
    clk = 1'b1;
    #1 $display("t2 %s %0.1f %0d %0d %0d %0d %0d", t, r, h == null, x, y, z, fired);
    g.w = 1'b0;
    clk = 1'b0;
    #1 -> e;
    g.w = 1'b1;
    clk = 1'b1;
    #1 $display("t4 %s %0.1f %0d %0d %0d %0d %0d", t, r, h == null, x, y, z, fired);
    g.w = 1'b0;
    clk = 1'b0;
    #1 -> e;
    g.w = 1'b1;
    clk = 1'b1;
    #1 $display("t6 %s %0.1f %0d %0d %0d %0d %0d", t, r, h == null, x, y, z, fired);
    $display("v %0d", h.v);
    $finish;
  end
endmodule
