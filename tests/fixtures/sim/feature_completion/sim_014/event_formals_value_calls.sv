// FIN-001 G3 (F06-06): output, inout and ref event formals of calls that yield
// a value: integral, real, string, handle, record and queue results, class
// event properties and event variables as actuals. See the readme.
`timescale 1ns/1ns
module tb;
  typedef struct { string s; int n; } R;
  typedef int Q[$];
  event g, h, r;
  event ge, gd;
  class C;
    event ev;
    function automatic string name(ref event e, output event o);
      o = e;
      e = ge;
      return "m";
    endfunction
    virtual function C self(output event o);
      o = ge;
      return this;
    endfunction
  endclass
  class D extends C;
    virtual function C self(output event o);
      o = gd;
      return null;
    endfunction
  endclass
  C c = new;
  C c2 = new;
  C o1 = new;
  C v;
  D d = new;

  function automatic int fo(output event o);
    o = g;
    return c.ev == g; // the actual changes at the return
  endfunction
  function automatic int fio(inout event o);
    int was = (o == g);
    o = h;
    return was;
  endfunction
  function automatic int fmove(output event o);
    C old = c;
    o = g;
    c = c2;
    return old == c;
  endfunction
  function automatic string fs(output event o, input string s);
    o = g;
    return {s, "!"};
  endfunction
  function automatic string fsr(ref event o, input string s);
    o = g;
    return {s, "?"};
  endfunction
  function automatic string fsio(inout event o);
    if (o == g) o = h; else o = g;
    return "io";
  endfunction
  function automatic string fsi(input event e);
    -> e;
    return "in";
  endfunction
  function automatic C fh(output event o, input C x);
    o = g;
    return x;
  endfunction
  function automatic C fhr(ref event o, input C x);
    o = g;
    return x;
  endfunction
  function automatic C fhi(input event e);
    -> e;
    return null;
  endfunction
  function automatic chandle fc(output event o);
    o = g;
    return null;
  endfunction
  function automatic real fr(output event o);
    o = g;
    return 1.5;
  endfunction
  function automatic real frr(ref event o);
    o = g;
    return 2.5;
  endfunction
  function automatic R fn(output event o);
    R x;
    x.s = "x";
    x.n = 3;
    o = g;
    return x;
  endfunction
  function automatic R fnr(ref event o);
    R x;
    x.s = "y";
    x.n = 4;
    o = g;
    return x;
  endfunction
  function automatic Q fq(output event o);
    o = g;
    return '{1, 2};
  endfunction
  function automatic Q fqr(ref event o);
    o = g;
    return '{3};
  endfunction

  int x;
  real y;
  R z;
  Q q;
  string s;
  C k;
  chandle ch;
  initial begin
    x = fo(c.ev);
    $display("int output %0d %0d", x, c.ev == g);
    x = fio(c.ev) + 10;
    $display("int inout %0d %0d %0d", x, c.ev == h, c.ev == g);
    fork
      begin @(c.ev) $display("%0t c.ev woke", $time); end
      begin #1 -> h; end
    join
    c = new;
    x = fmove(c.ev);
    $display("moved %0d %0d", x, c.ev == g);
    c = new;
    s = fs(h, "a");
    $display("string %s %0d", s, h == g);
    s = fs(c.ev, "b");
    $display("string %s %0d", s, c.ev == g);
    h = null;
    s = fsr(r, "c");
    $display("string %s %0d", s, r == g);
    c.ev = null;
    s = fsr(c.ev, "d");
    $display("string %s %0d", s, c.ev == g);
    s = fsio(c.ev);
    $display("string %s %0d %0d", s, c.ev == g, c.ev == h);
    h = c2.ev;
    s = fsio(c.ev);
    $display("string %s %0d", s, c.ev == h);
    fork
      begin @(g) $display("%0t g woke", $time); end
      begin #1 s = fsi(g); end
    join
    fork
      begin @(g) $display("%0t g woke", $time); end
      begin #1 k = fhi(g); end
    join
    h = null;
    k = fh(h, o1);
    $display("handle %0d %0d", k == o1, h == g);
    r = null;
    k = fhr(r, o1);
    $display("handle %0d %0d", k == o1, r == g);
    c.ev = null;
    k = fh(c.ev, o1);
    $display("handle %0d %0d", k == o1, c.ev == g);
    c.ev = null;
    k = fhr(c.ev, o1);
    $display("handle %0d %0d", k == o1, c.ev == g);
    c.ev = null;
    ch = fc(c.ev);
    $display("chandle %0d %0d", ch == null, c.ev == g);
    c.ev = null;
    y = fr(c.ev);
    $display("real %0.1f %0d", y, c.ev == g);
    c.ev = null;
    y = frr(c.ev);
    $display("real %0.1f %0d", y, c.ev == g);
    c.ev = null;
    z = fn(c.ev);
    $display("record %s %0d %0d", z.s, z.n, c.ev == g);
    c.ev = null;
    z = fnr(c.ev);
    $display("record %s %0d %0d", z.s, z.n, c.ev == g);
    c.ev = null;
    q = fq(c.ev);
    $display("queue %0d %0d", q.size(), c.ev == g);
    c.ev = null;
    q = fqr(c.ev);
    $display("queue %0d %0d", q.size(), c.ev == g);
    r = null;
    c.ev = null;
    s = c.name(r, c.ev);
    $display("method %s %0d %0d", s, c.ev == ge, r == ge);
    v = c;
    k = v.self(h);
    $display("virtual %0d %0d", k == c, h == ge);
    v = d;
    k = v.self(h);
    $display("virtual %0d %0d", k == null, h == gd);
    $finish;
  end
endmodule
