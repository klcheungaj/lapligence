`timescale 1ns/1ns
class C; int id; function new(int i); id=i; endfunction endclass
class M; int v; function new(int x); v=x; endfunction function int get(); return v; endfunction endclass
function automatic void bump(ref int x); x += 100; endfunction
function automatic void setstr(ref string x); x = "set"; endfunction
task automatic slow(ref int a[C], input C k);
  foreach (a[z]) begin
    #1 $display("slow %0d %0d %0t", z.id, a[z], $time);
    if (z == k) a[z] = 99;
  end
endtask
module tb;
  C c[6];
  int a[C], b[C];
  string s[C];
  M mm[C];
  int w[C];
  C oc[C];
  logic [3:0] lv[C];
  int pos;
  int qq[$];
  C cq[$];
  initial begin
    C k;
    foreach (c[i]) c[i] = new(i);
    a[c[1]] = 10; a[c[2]] = 20; a[c[3]] = 30;
    bump(a[c[2]]);
    $display("%0d", a[c[2]]);
    bump(a[c[4]]);
    $display("%0d %0d", a.exists(c[4]), a[c[4]]);
    s[c[1]] = "x"; setstr(s[c[1]]); setstr(s[c[2]]);
    $display("%s %s", s[c[1]], s[c[2]]);
    mm[c[2]] = new(5); mm[c[1]] = new(6);
    $display("%0d %0d", mm[c[2]].get(), mm[c[1]].v);
    mm[c[2]].v = 8;
    $display("%0d", mm[c[2]].v);
    lv[c[1]] = 4'b10x1; lv[c[1]][2] = 1'b0;
    $display("%b", lv[c[1]]);
    qq = a.min(); $display("sum %0d min %0d", a.sum(), qq[0]);
    qq = a.find with (item > 20);
    $display("%0d %0d", qq.size(), qq[0]);
    qq = a.find_first with (item > 20);
    $display("%0d %0d", qq.size(), qq[0]);
    qq = a.unique();
    $display("%0d", qq.size());
    $display("%0d %0d", qq.size(), qq[0]);
    a.delete(c[4]);
    slow(a, c[2]);
    $display("%0d", a[c[2]]);
    fork
      begin wait (w[c[1]] == 2); $display("waited %0t", $time); end
      #5 w[c[1]] = 2;
    join_none
    #10 $display("%0t", $time);
    $finish;
  end
endmodule
