// Functions returning an event: the result is the handle the function
// returns (SV 13.4.1, 15.5.5). See the readme.
`timescale 1ns/1ns
module tb;
  class C;
    event ev;
    function event get();
      return ev;
    endfunction
    static function event sget(event e);
      return e;
    endfunction
  endclass
  function automatic event pass(event f);
    return f;
  endfunction
  function automatic event pick(int i, event u, event v);
    if (i == 0) return u;
    return v;
  endfunction
  function automatic event fnull();
    return null;
  endfunction
  function automatic event fdef();
  endfunction
  function event fstat(event f);
    fstat = f;
  endfunction
  function automatic event trig(event f);
    trig = f;
    -> trig;
  endfunction
  task automatic t_wait(input event e);
    @(e) $display("%0d t_wait woke", $time);
  endtask
  C c;
  event a, b, d, j, k, m, n, e1, e2, p, q, r, s, x, y;
  initial begin
    c = new;
    j = pass(k);
    m = fstat(d);
    n = fnull();
    e1 = fdef();
    e2 = fdef();
    p = pass(pass(a));
    x = c.get();
    y = C::sget(x);
    $display("j==k %0d m==d %0d n==null %0d p==a %0d", j == k, m == d, n == null, p == a);
    $display("pass(k)==k %0d pass(k)==null %0d fnull()==null %0d fdef()==null %0d",
             pass(k) == k, pass(k) == null, fnull() == null, fdef() == null);
    $display("e1==e2 %0d pick0==a %0d pick1==a %0d", e1 == e2, pick(0, a, b) == a, pick(1, a, b) == a);
    $display("x==c.ev %0d y==x %0d c.get()==c.ev %0d", x == c.ev, y == x, c.get() == c.ev);
    if (pass(a)) $display("pass(a) is true");
    c.ev = pass(s);
    $display("c.ev==s %0d", c.ev == s);
  end
  initial #1 forever begin @(j) $display("%0d woke j", $time); end
  initial #1 forever begin @(m) $display("%0d woke m", $time); end
  initial #1 forever begin @(e2) $display("%0d woke e2", $time); end
  initial #1 forever begin @(pass(a)) $display("%0d woke @pass(a)", $time); end
  initial #1 forever begin @(pick(1, a, b) or r) $display("%0d woke @(pick(1,a,b) or r)", $time); end
  initial #1 forever begin @(c.get()) $display("%0d woke @c.get()", $time); end
  initial begin
    #2 -> k;
    #1 -> d;
    #1 -> e1;
    #1 -> a;
    #1 -> b;
    #1 -> r;
    #1 -> s;
    #1 fork
      t_wait(pass(q));
      #1 -> q;
    join
    #1 void'(trig(r));
    #1 $finish;
  end
endmodule
