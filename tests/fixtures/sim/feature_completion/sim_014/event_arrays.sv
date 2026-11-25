`timescale 1ns/1ns
package p;
  event pa, pb;
  event pe[2] = '{pa, pb};
endpackage

module tb;
  import p::*;
  event a, b, n;
  event ea[2] = '{a, b};
  event eb[2];
  event ec[1:0] = '{default: a};
  event em[2][2] = '{'{a, b}, '{default: n}};
  event en[2][2];
  event ek[2] = '{1: a, 0: b};
  event er[2] = '{2{a}};
  event ed[] = '{a, b};
  event eq[$] = '{a, b, null};
  event es[string] = '{"x": a, "y": b};

  class holder;
    event f[2] = '{a, b};
    event g[] = '{a, b};
    event q[$] = '{b};
    event m[int];
    static event sf[2] = '{b, a};
    function bit same();
      return f[1] == b && g[0] == a && q[0] == b;
    endfunction
  endclass
  holder o;

  function bit static_array();
    static event s[2] = '{a, b};
    return s[1] == b;
  endfunction
  task automatic locals(int k);
    event s[2];
    event d[2] = '{a, b};
    event q[$] = '{a, s[1]};
    static event t[2] = '{b, a};
    $display("local %b %b %b %b", s[k] == null, d[1] == b, q[1] == s[1], t[k] == (k ? a : b));
    s = '{d[1], d[0]};
    $display("local %b %b", s[0] == b, s[1] == a);
  endtask
  task automatic waits(event e, string name);
    @(e);
    $display("%s %0t", name, $time);
  endtask

  initial fork
    begin @(ea[0]); $display("ea0 %0t", $time); end
    begin @(ea[1]); $display("ea1 %0t", $time); end
    waits(ec[0], "ec0");
    waits(em[0][1], "em01");
    waits(ed[1], "ed1");
    waits(eq[0], "eq0");
    waits(es["y"], "esy");
  join_none

  initial begin
    $display("%b %b %b %b", em[1][1] == n, eq[2] == null, ea[0] == a, ea == ec);
    $display("%b %b %b %b", pe[0] == pa, ek[0] == b, ek[1] == a, er[1] == a);
    en = em;
    $display("%b", en == em);
    en[0] = em[1];
    $display("%b %b", en[0][0] == n, en != em);
    en[1] = '{b, a};
    $display("%b", en[1][0] == b);
    ek = '{ek[1], ek[0]};
    $display("%b %b", ek[0] == a, ek[1] == b);
    $display("%b %b %b", ek == ea, ek !== ea, ek === er);
    $display("%b", static_array());
    locals(0);
    locals(1);
    for (int i = 0; i < 2; i++) begin
      automatic event l[2] = '{a, b};
      event st[2];
      l[i] = n;
      $display("block %b %b %b", l[0] == n, l[1] == n, st[i] == null);
    end
    o = new;
    $display("%b %b %b", o.same(), o.f[1] == b, holder::sf[0] == b);
    o.f[1] = n;
    o.q.push_back(o.f[0]);
    $display("%b %b", o.f[1] == n, o.q[1] == a);
    #1 -> a;
    #1 -> b;
    #1 eb = '{b, a};
    fork
      waits(eb[0], "eb0");
      waits(o.f[1], "of1");
      begin @(o.m[2]); $display("m2 %0t", $time); end
    join_none
    #1 -> b;
    #1 -> o.f[1];
    #1 -> o.m[2];
    #1 eb = ea;
    $display("%b", eb == ea);
    $finish;
  end
endmodule
