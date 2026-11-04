// Sources of nonblocking record writes with container members.
module tb;
  typedef struct { int k; string v; } in_t;
  typedef struct { string s; int q[$]; string e[$]; real f[]; string t[string]; } rec_t;
  rec_t m, n, p;
  typedef struct { string s; int q[$]; } lit_t;
  lit_t a1, a2, o;
  event ev;
  bit c = 1;
  function automatic rec_t mk(string s);
    rec_t r;
    r.s = s; r.q.push_back(4); r.q.push_back(5);
    r.e.push_back("e1");
    r.f = new[1]; r.f[0] = 2.5;
    r.t["a"] = "b";
    return r;
  endfunction
  initial begin
    n <= mk("call");
    #1 $display("[%s] %0d %s %0.1f %s", n.s, n.q.size(), n.e[0], n.f[0], n.t["a"]);
    m <= '{s:"pat", q:'{1, 2, 3}, e:'{"p"}, f:'{8.5}, t:'{"x":"y"}};
    #1 $display("[%s] %0d %s %0.1f %s", m.s, m.q.size(), m.e[0], m.f[0], m.t["x"]);
    a1.s = "a1"; a1.q.push_back(6); a2.s = "a2";
    o <= c ? a1 : a2;
    #1 $display("[%s] %0d", o.s, o.q.size());
    n.q <= '{9};
    n.t <= m.t;
    #1 $display("%0d %0d %s", n.q.size(), n.t.size(), n.t["x"]);
    p <= n; p.s <= "over";
    #1 $display("[%s] %0d", p.s, p.q.size());
    $finish(0);
  end
endmodule
