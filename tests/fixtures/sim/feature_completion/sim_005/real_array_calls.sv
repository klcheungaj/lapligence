// SIM-005: real and shortreal fixed arrays cross subroutine boundaries as
// numeric cells (IEEE 1800-2009 7.6, 7.4.6, 11.4.11, 13.3-13.5).
module tb;
  typedef real ra_t[3];
  typedef real rm_t[2][2];
  typedef shortreal sa_t[3];
  ra_t a, b, c;
  rm_t m, n;
  sa_t s;
  bit sel;
  logic xsel;

  function automatic ra_t scale(input ra_t x, input real k);
    ra_t y;
    for (int i = 0; i < 3; i++) y[i] = x[i] * k;
    return y;
  endfunction
  function automatic real total(input ra_t x);
    real t;
    t = 0.0;
    foreach (x[i]) t += x[i];
    // The input is a private copy.
    x[0] = 1000.0;
    return t;
  endfunction
  function automatic rm_t transpose(input rm_t x);
    rm_t y;
    foreach (x[i, j]) y[j][i] = x[i][j];
    return y;
  endfunction
  function automatic real row_sum(input real r[2]);
    return r[0] + r[1];
  endfunction
  function static real mean(input real v);
    static real acc[2];
    acc[0] = acc[0] + v;
    acc[1] = acc[1] + 1.0;
    return acc[0] / acc[1];
  endfunction
  task automatic doubled(inout ra_t x);
    foreach (x[i]) x[i] = x[i] * 2.0;
  endtask
  task automatic negate_mid(ref ra_t x);
    x[1] = -x[1];
  endtask
  task automatic thirds(output sa_t o, input shortreal base);
    foreach (o[i]) o[i] = shortreal'(real'(base) / 3.0 * real'(i + 1));
  endtask
  task automatic slow_fill(output ra_t o);
    #2 o = '{7.0, 8.0, 9.0};
  endtask

  initial begin
    a = '{1.5, 2.5, 3.5};
    b = scale(a, 2.0);
    $display("scale %.2f %.2f %.2f", b[0], b[1], b[2]);
    $display("total %.2f %.2f %.2f", total(a), total('{1.0, 2.0, 4.0}), a[0]);
    doubled(b);
    $display("inout %.2f %.2f %.2f", b[0], b[1], b[2]);
    negate_mid(b);
    $display("ref %.2f %.2f %.2f", b[0], b[1], b[2]);
    c = a;
    $display("eq %0d %0d %0d", a == c, a != b, a == b);
    sel = 1;
    c = sel ? a : b;
    $display("cond1 %.2f %.2f", c[0], c[1]);
    sel = 0;
    c = sel ? a : b;
    $display("cond0 %.2f %.2f", c[0], c[1]);
    // An ambiguous predicate keeps equal elements and yields 0.0 otherwise.
    xsel = 1'bx;
    b = '{1.5, 9.0, 3.5};
    c = xsel ? a : b;
    $display("condx %.2f %.2f %.2f", c[0], c[1], c[2]);
    m = '{'{1.0, 2.0}, '{3.0, 4.0}};
    n = transpose(m);
    $display("transpose %.1f %.1f %.1f %.1f", n[0][0], n[0][1], n[1][0], n[1][1]);
    $display("row %.2f", row_sum(m[1]));
    m = transpose(transpose(n));
    $display("nested %0d %.1f", m == n, m[0][1]);
    $display("static %.2f %.2f %.2f", mean(1.0), mean(2.0), mean(6.0));
    thirds(s, shortreal'(1.0));
    $display("short %h %h %h", $shortrealtobits(s[0]), $shortrealtobits(s[1]),
             $shortrealtobits(s[2]));
    void'(scale(a, 3.0));
    a = scale(a, 0.5);
    $display("self %.3f %.3f %.3f", a[0], a[1], a[2]);
    slow_fill(c);
    $display("timed %0d %.1f %.1f %.1f", $time, c[0], c[1], c[2]);
    $finish(0);
  end
endmodule
