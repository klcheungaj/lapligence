`timescale 1ns/1ns
class C; int id; int pad[]; function new(int i); id=i; pad=new[4]; endfunction endclass
class H; int m[C]; string s[C]; int n[string][C]; int q[$][C]; endclass
module tb;
  int a[C];
  string st[C];
  int nest[string][C];
  int qa[$][C];
  int ca[C][C];
  H h;
  function automatic void fill(int base);
    C t;
    for (int i = 0; i < 4; i++) begin
      t = new(base + i);
      a[t] = i;
      st[t] = "s";
      nest["x"][t] = i;
      h.m[t] = i;
      h.s[t] = "h";
      h.n["y"][t] = i;
      ca[t][t] = i;
    end
  endfunction
  task automatic churn();
    C t;
    for (int i = 0; i < 200; i++) begin t = new(-1); #1; end
  endtask
  task automatic local_keys();
    int l[C];
    string sl[C];
    C t;
    for (int i = 0; i < 3; i++) begin t = new(500 + i); l[t] = i; sl[t] = "l"; end
    t = null;
    churn();
    foreach (l[k]) $display("local %0d %0d %s", k.id, l[k], sl[k]);
  endtask
  initial begin
    int b[C];
    h = new;
    fill(100);
    begin
      C t = new(200);
      int e[C];
      e[t] = 1;
      qa.push_back(e);
    end
    churn();
    foreach (a[k]) $display("a %0d %0d %s", k.id, a[k], st[k]);
    foreach (h.m[k]) $display("hm %0d %s", k.id, h.s[k]);
    foreach (ca[k, k2]) $display("ca %0d %0d", k.id, k2.id);
    foreach (qa[i, k]) $display("qa %0d %0d", i, k.id);
    local_keys();
    b = a;
    a.delete();
    churn();
    foreach (b[k]) $display("b %0d", k.id);
    $finish;
  end
endmodule
