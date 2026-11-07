`timescale 1ns/1ps
class B; int v; function new(int x=0); v=x; endfunction endclass
class D extends B; function new(int x=0); super.new(x); endfunction endclass
module tb;
  int w[B];
  int n[B][string];
  int q[B][$];
  int r[string][B];
  B bb; D dd;
  B k1,k2,k3;
  initial begin
    k1=new(1); k2=new(2); k3=new(3);
    bb=k1;
    w[k1]=1; w[k2]=2; w[k3]=3;
    dd=new(4);
    w[dd]=4;
    $display("%0d %0d", w.num(), w.size());
    $display("%0d", w.exists(dd));
    bb=dd;
    $display("%0d", w[bb]);
    w.delete(k2);
    w.delete(dd);
    $display("%0d %0d", w.num(), w.exists(k2));
    foreach (w[k]) $display("v=%0d val=%0d", k.v, w[k]);
    n[k1]["a"]=1; n[k1]["b"]=2; n[k2]["c"]=3;
    foreach (n[i,j]) $display("%0d %s %0d", i.v, j, n[i][j]);
    q[k1].push_back(5); q[k1].push_back(6); q[k3].push_back(7);
    foreach (q[i,j]) $display("%0d %0d %0d", i.v, j, q[i][j]);
    r["x"][k1]=9; r["x"][k3]=8;
    foreach (r[s,c]) $display("%s %0d %0d", s, c.v, r[s][c]);
    $display("%0d %0d", n.num(), n[k1].num());
    $display("%0d", q.num());
    begin B c; if (w.first(c)) do $display("it %0d", c.v); while (w.next(c)); 
      if (w.last(c)) do $display("rit %0d", c.v); while (w.prev(c)); end
    w[null]=11; $display("%0d %0d", w.exists(null), w[null]);
    w.delete(); $display("%0d",w.num());
    $finish(0);
  end
endmodule
