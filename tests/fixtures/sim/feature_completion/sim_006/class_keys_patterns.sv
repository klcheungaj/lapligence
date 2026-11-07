`timescale 1ns/1ns
class C; int id; function new(int i); id=i; endfunction endclass
module tb;
  C c[4];
  int a[C], b[C], m[C];
  string s[C];
  C o[C];
  real r[C];
  bit f = 1;
  int q[$];
  int nq[$][C];
  initial begin
    foreach (c[i]) c[i] = new(i);
    a[c[2]] = 5; a[c[1]] = 6;
    foreach (a[k]) $display("a %0d %0d", k.id, a[k]);
    a = '{null: 3, default: 9};
    $display("%0d %0d %0d", a[null], a[c[1]], a.num());
    b[c[1]] = 1;
    m = f ? a : b;
    $display("%0d %0d", m.num(), m[null]);
    m = !f ? a : b;
    $display("%0d %0d", m.num(), m[c[1]]);
    m = 1'bx ? a : b;
    $display("%0d", m.num());
    nq = {};
    nq.push_back('{null: 4});
    $display("%0d %0d", nq.size(), nq[0][null]);
    $finish;
  end
endmodule
