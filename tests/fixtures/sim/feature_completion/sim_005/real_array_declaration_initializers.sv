// Declaration initializers of multidimensional real and shortreal arrays
// (module, package and automatic task storage) and of arrays whose cells
// are strings, chandles or records with a real member.
package pk;
  real pr[2][2] = '{'{1.0, 2.0}, '{3.0, 4.0}};
  shortreal ps[2][3] = '{'{0.5, 1.5, 2.5}, '{3.5, 4.5, 5.5}};
endpackage

module tb;
  typedef struct { real a; int b; } rs_t;
  real x = 1.5;
  parameter real P = 2.5;
  real r2[2][2] = '{'{1.0, 2.0}, '{3.0, 4.0}};
  real r3[2][2][2] = '{'{'{1.0, 2.0}, '{3.0, 4.0}}, '{'{5.0, 6.0}, '{7.0, 8.0}}};
  shortreal s2[2][2] = '{'{1.25, 2.25}, '{3.25, 4.25}};
  real rd[2][2] = '{default: 7.5};
  real rr[1:0][0:1] = '{'{1.0, 2.0}, '{3.0, 4.0}};
  real n1[2] = '{x, P};
  real n2[2][2] = '{'{x, P}, '{P, x}};
  real cp[2][2] = r2;
  real rep[2][3] = '{2{'{3{0.5}}}};
  real mix[2][2] = '{'{1, 2}, '{3'd3, 4.5}};
  shortreal sh[2][2] = '{'{x, P}, '{1.0, 0.1}};
  real idx[2][2] = '{0: '{1: 9.0, default: 1.0}, 1: '{2.0, 3.0}};
  function automatic real f(real a);
    return a * 2;
  endfunction
  real fc[2][2] = '{'{f(1.0), f(x)}, '{f(P), 0.0}};
  string st2[2][2] = '{'{"a", "b"}, '{"c", "d"}};
  chandle ch2[2][2] = '{'{null, null}, '{null, null}};
  rs_t rs2[2][2] = '{'{'{1.5, 1}, '{2.5, 2}}, '{'{3.5, 3}, '{4.5, 4}}};

  task automatic t(int k);
    real lr[2][2] = '{'{1.5, 2.5}, '{3.5, 4.5 + k}};
    string ls[2][2] = '{'{"w", "x"}, '{"y", "z"}};
    rs_t lrs[2][2] = '{'{'{0.5, 5}, '{0.25, 6}}, '{'{0.125, 7}, '{1.0, 8}}};
    $display("lr %0.2f %0.2f %0.2f %0.2f", lr[0][0], lr[0][1], lr[1][0], lr[1][1]);
    $display("ls %s %s %s %s", ls[0][0], ls[0][1], ls[1][0], ls[1][1]);
    $display("lrs %0.3f %0d %0.3f %0d", lrs[0][1].a, lrs[0][1].b, lrs[1][0].a, lrs[1][1].b);
    lr[1][1] = 0.0;
  endtask

  initial begin
    $display("r2 %0.1f %0.1f %0.1f %0.1f", r2[0][0], r2[0][1], r2[1][0], r2[1][1]);
    $display("r3 %0.1f %0.1f %0.1f", r3[0][0][1], r3[1][0][0], r3[1][1][1]);
    $display("s2 %0.2f %0.2f %0.2f %0.2f", s2[0][0], s2[0][1], s2[1][0], s2[1][1]);
    $display("rd %0.1f %0.1f", rd[0][0], rd[1][1]);
    $display("rr %0.1f %0.1f %0.1f %0.1f", rr[1][0], rr[1][1], rr[0][0], rr[0][1]);
    $display("n1 %0.2f %0.2f", n1[0], n1[1]);
    $display("n2 %0.2f %0.2f %0.2f %0.2f", n2[0][0], n2[0][1], n2[1][0], n2[1][1]);
    $display("cp %0.2f %0.2f", cp[0][1], cp[1][0]);
    $display("rep %0.2f %0.2f", rep[0][2], rep[1][1]);
    $display("mix %0.2f %0.2f %0.2f %0.2f", mix[0][0], mix[0][1], mix[1][0], mix[1][1]);
    $display("sh %0.2f %0.2f %0.8f", sh[0][1], sh[1][0], sh[1][1]);
    $display("idx %0.2f %0.2f %0.2f %0.2f", idx[0][0], idx[0][1], idx[1][0], idx[1][1]);
    $display("fc %0.2f %0.2f %0.2f %0.2f", fc[0][0], fc[0][1], fc[1][0], fc[1][1]);
    $display("pr %0.1f %0.1f", pk::pr[0][1], pk::pr[1][0]);
    $display("ps %0.1f %0.1f", pk::ps[0][2], pk::ps[1][1]);
    $display("st2 %s %s %s %s", st2[0][0], st2[0][1], st2[1][0], st2[1][1]);
    $display("ch2 %0d", ch2[1][1] == null);
    $display("rs2 %0.1f %0d %0.1f %0d", rs2[0][1].a, rs2[0][1].b, rs2[1][0].a, rs2[1][1].b);
    t(1);
    t(2);
    $finish(0);
  end
endmodule
