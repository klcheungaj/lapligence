class C; int id; function new(int i); id=i; endfunction endclass
class D extends C; function new(int i); super.new(i); endfunction endclass
class H;
  int m[C];
  string s[C];
  C o[C];
  real r[C];
  int q[C][$];
  int n[C][string];
  int w[string][C];
  int cc[C][C];
  function void add(C k, int v); m[k]=v; s[k]=$sformatf("v%0d",v); o[k]=k; r[k]=v/2.0; q[k].push_back(v); n[k]["a"]=v; w["x"][k]=v; cc[k][k]=v; endfunction
  function void show();
    C k;
    if (m.first(k)) do $write(" %0d:%0d/%s/%0d/%f/%0d/%0d/%0d/%0d", k.id, m[k], s[k], o[k].id, r[k], q[k].size(), n[k]["a"], w["x"][k], cc[k][k]); while (m.next(k));
    $display("");
  endfunction
endclass
function automatic int sum_keys(int a[C]);
  int t = 0;
  foreach (a[k]) t += k.id * a[k];
  return t;
endfunction
function automatic void fill(ref int a[C], C k);
  a[k] = 3;
endfunction
function automatic int only(output int a[C], input C k);
  a[k] = 9;
  return a.num();
endfunction
function automatic C pick(int a[C]);
  C k;
  void'(a.last(k));
  return k;
endfunction
module tb;
  H h = new;
  C c[6];
  D d = new(50);
  int a[C];
  int b[C];
  bit vis[C];
  initial begin
    C k, k2;
    foreach (c[i]) c[i] = new(i);
    h.add(c[2],20); h.add(c[1],10); h.add(c[5],5);
    a[d] = 50; $display("derived %0d %0d", a.exists(d), a[d]); a.delete(d);
    h.show();
    a[c[3]] = 3; a[c[0]] = 1; a[c[2]] = 7;
    b = a;
    b[c[1]] = 2;
    $display("%0d %0d", a.num(), b.size());
    $display("%0d", sum_keys(b));
    fill(a, c[4]);
    $display("%0d", a[c[4]]);
    $display("%0d", only(b, c[5]));
    $display("%0d", b.num());
    $display("%0d", pick(a).id);
    a[c[3]]++;
    a[c[3]] += 10;
    #1 $display("%0d %0d", a[c[3]], a[c[0]]);
    $display("exists %0d %0d", a.exists(c[5]), a.exists(c[3]));
    a.delete(c[3]);
    $display("%0d %0d", a.num(), a.exists(c[3]));
    $display("miss %0d", a[c[5]]);
    $display("exists after read %0d", a.exists(c[5]));
    a.delete();
    $display("%0d", a.num());
    begin
      int z[C] = '{default: 4};
      $display("dflt %0d %0d", z[c[1]], z.num());
      z[null] = 8; 
      $display("%0d %0d", z[null], z.num());
    end
    vis[c[1]] = 1; vis[c[1]] = 0;
    $display("%0d %0d", vis.num(), vis[c[1]]);
    foreach (h.m[kk]) $display("fm %0d", kk.id);
    foreach (h.n[kk, str]) $display("fn %0d %s", kk.id, str);
    foreach (h.q[kk, i]) $display("fq %0d %0d", kk.id, i);
    foreach (h.cc[k1, k2x]) $display("fc %0d %0d", k1.id, k2x.id);
    $display("%0d", h.n[c[1]].num());
    $display("%0d", h.q[c[1]].size());
    $display("%0d", h.cc[c[1]].exists(c[1]));
    h.cc[c[1]].delete(c[1]);
    $display("%0d %0d", h.cc[c[1]].exists(c[1]), h.cc.exists(c[1]));
    if (h.w["x"].first(k)) $display("wf %0d", k.id);
    if (h.w["x"].next(k)) $display("wn %0d", k.id);
    
    $finish;
  end
endmodule
