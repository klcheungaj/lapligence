module tb;
  typedef struct { int p; string q; } pair;
  typedef struct { int m[int]; int k; string s; } keyed;
  typedef struct { logic [3:0] v[string]; } inner;
  typedef struct { inner in; int n; } outer;
  class C;
    int a[int];
    function bit same(C other);
      return a == other.a;
    endfunction
  endclass

  int a[int], b[int];
  string s[string], t[string];
  logic [3:0] l[int], m[int];
  int w[*], x[*];
  real r[int], u[int];
  pair pa[int], pb[int];
  int qa[int][$], qb[int][$];
  int fa[int][2], fb[int][2];
  int na[int][string], nb[int][string];
  logic [1:0] xa[int][string], xb[int][string];
  event ea[int], eb[int];
  C ca[C], cb[C];
  C o = new, p = new;
  keyed ka, kb, kc;
  outer oa, ob;
  logic pick;
  bit comb;
  int woke;

  always_comb comb = a == b;

  function automatic bit equal(int f[int], int g[int]);
    return f == g;
  endfunction
  function automatic bit grown(keyed first, keyed second);
    keyed local_copy;
    local_copy = first;
    local_copy.m[1] = 2;
    return local_copy == second;
  endfunction

  initial begin
    wait (a == b && a.size() == 2);
    woke = 1;
  end

  initial begin
    $display("empty %b %b", a == b, a != b);
    a[1] = 2;
    $display("size %b %b", a == b, a != b);
    b[1] = 2;
    #1 $display("equal %b %b %b %b comb %b", a == b, a != b, a === b, a !== b, comb);
    b[2] = 3;
    a[3] = 3;
    $display("keys %b", a == b);
    a.delete(3);
    a[2] = 4;
    $display("value %b", a == b);
    #1 a[2] = 3;
    #1 $display("woke %0d", woke);
    s["x"] = "1";
    t["x"] = "1";
    $display("string %b", s == t);
    t["x"] = "2";
    $display("string %b", s == t);
    l[1] = 4'b1x01;
    m[1] = 4'b1x01;
    $display("unknown %b %b %b", l == m, l === m, l != m);
    m[1] = 4'b0x01;
    $display("unknown %b %b", l == m, l !== m);
    r[1] = 1.5;
    u[1] = 1.5;
    $display("real %b", r == u);
    w[1] = 2;
    x[1] = 2;
    $display("wildcard %b", w == x);
    x[2] = 2;
    $display("wildcard %b", w == x);
    o.a = a;
    p.a = a;
    $display("property %b %b %b", o.a == p.a, o.a == a, o.same(p));
    p.a[9] = 1;
    $display("property %b %b", o.same(p), equal(a, b));
    pa[1] = '{1, "a"};
    pb[1] = '{1, "a"};
    $display("record %b", pa == pb);
    pb[1].q = "b";
    $display("record %b", pa == pb);
    qa[1] = '{1, 2};
    qb[1] = '{1, 2};
    $display("queue %b", qa == qb);
    qb[1].push_back(1);
    $display("queue %b", qa == qb);
    fa[1] = '{1, 2};
    fb[1] = '{1, 2};
    $display("fixed %b", fa == fb);
    na[1]["a"] = 2;
    $display("nested %b", na == nb);
    nb[1]["a"] = 2;
    $display("nested %b %b", na == nb, na[1] == nb[1]);
    xa[1]["a"] = 2'b1x;
    xb[1]["a"] = 2'b1x;
    $display("nested unknown %b %b", xa == xb, xa === xb);
    $display("handles %b %b", ea == eb, ca == cb);
    ca[o] = p;
    cb[o] = p;
    $display("handles %b", ca == cb);
    ea[1] = null;
    $display("handles %b", ea == eb);

    ka.m[1] = 2;
    $display("member %b", ka == kb);
    kb.m[1] = 2;
    $display("member %b %b %b", ka == kb, ka != kb, ka.m == kb.m);
    $display("member %b", grown(kc, ka));
    kb.m[2] = 1;
    $display("member %b", ka === kb);
    kc = pick ? ka : ka;
    $display("merge %0d", kc.m.num());
    oa.in.v["a"] = 3;
    $display("inner %b", oa == ob);
    ob.in.v["a"] = 3;
    $display("inner %b", oa == ob);
    if (a == b) $display("same");
    $finish(0);
  end
endmodule
