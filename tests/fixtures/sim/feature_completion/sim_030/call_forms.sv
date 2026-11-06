// Constraint functions (18.5.11) with a loop index, an array element or
// another object's variable as argument, and a function of an object reached
// through a `rand` handle or a handle array element, which reads its own
// object's state. Each occurrence is one call; arguments are solved before
// the call, so elements chain through a function.
class leaf_c;
  rand bit [3:0] v;
  rand bit [3:0] w;
  int base;
  function new(int b); base = b; endfunction
  function int inc(int x); return x + base; endfunction
  constraint c { w == inc(v) % 16; v < 10; }
endclass

class top_c;
  rand bit [7:0] a[4];
  rand bit [7:0] d[];
  rand bit [3:0] k;
  rand leaf_c h[2];
  rand leaf_c s;
  int tbl[4] = '{5, 6, 7, 8};
  function int f(int i); return tbl[i] * 2; endfunction
  function int g(bit [7:0] x); return x + 1; endfunction
  constraint c {
    foreach (a[i]) a[i] == f(i);
    d.size() == 3;
    foreach (d[i]) i > 0 -> d[i] == g(d[i - 1]);
    d[0] < 10;
    k == g(s.v) % 16;
  }
  function new();
    s = new(2);
    h[0] = new(3);
    h[1] = new(4);
  endfunction
endclass

module tb;
  initial begin
    static top_c m = new;
    int starts[10];
    int bad;
    repeat (200) begin
      if (m.randomize() != 1) $display("failed");
      foreach (m.a[i]) if (m.a[i] != m.tbl[i] * 2) bad++;
      if (m.d[1] != m.d[0] + 1 || m.d[2] != m.d[1] + 1 || m.d[0] >= 10) bad++;
      starts[m.d[0]]++;
      if (m.k != (m.s.v + 1) % 16) bad++;
      if (m.s.w != (m.s.v + 2) % 16) bad++;
      if (m.h[0].w != (m.h[0].v + 3) % 16) bad++;
      if (m.h[1].w != (m.h[1].v + 4) % 16) bad++;
    end
    $display("bad=%0d %p", bad, m.a);
    foreach (starts[i]) if (starts[i] == 0) $display("missing %0d", i);
    $finish(0);
  end
endmodule
