// Fixed arrays of reals and events that are class properties, used whole
// and by element through a handle.
`timescale 1ns / 1ns
module tb;
  class c;
    real r[2];
    real d[3:1];
    event ea[2:1];
    static event sa[1:2];

    function bit same(real t[2]);
      return r == t;
    endfunction

    function real first();
      real t[2];
      t = r;
      return t[0];
    endfunction
  endclass

  c o, p;
  real t[2];
  real u[3:1];
  event a;
  int i;

  initial begin
    o = new;
    p = new;
    o.r[0] = 1.5;
    o.r[1] = 2.25;
    o.d[2] = 4.0;
    t = o.r;
    $display("%g %g %p", t[0], t[1], o.r);
    $display("%b %b %b", o.r == t, o.same(t), t != o.r);
    o.r = '{3.0, 4.0};
    u = o.d;
    $display("%p %p %g", o.r, u, o.first());
    u[3] = 7.5;
    o.d = u;
    $display("%p %g %b", o.d, o.d[3], o.d == u);
    i = 2;
    o.ea[1] = a;
    $display("%b %b %b", o.ea[1] == a, o.ea[i] == a, o.ea[1] != p.ea[1]);
    fork
      begin
        @(o.ea[i]);
        $display("ea[2] %0t", $time);
      end
      begin
        wait (o.ea[1].triggered);
        $display("ea[1] %0t", $time);
      end
      begin
        @(c::sa[2]);
        $display("sa[2] %0t", $time);
      end
    join_none
    #1 -> a;
    #1 ->> o.ea[2];
    #1 -> c::sa[i];
    #1 p.ea[2] = o.ea[2];
    $display("%b", p.ea[2] == o.ea[2]);
    $finish;
  end
endmodule
