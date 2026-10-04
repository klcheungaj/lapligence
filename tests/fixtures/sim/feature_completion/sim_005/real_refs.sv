// SIM-005: real/shortreal ref and const ref formals alias the selected
// numeric storage cell (IEEE 1800-2009 13.5.2, 6.12).
module tb;
  typedef struct { real r; int k; shortreal s; } rec_t;
  real r, acc, lo, hi;
  shortreal s;
  real a[3];
  real m[2][3];
  rec_t rec;
  int idx, hits;

  task automatic add(ref real x, input real d);
    x = x + d;
  endtask
  task automatic forward(ref real y);
    add(y, 1.0);
    add(y, 0.25);
  endtask
  task automatic triple(ref shortreal x);
    x = shortreal'(real'(x) * 3.0);
  endtask
  function automatic real twice(const ref real x);
    return 2.0 * x;
  endfunction
  function automatic real look(const ref real x);
    return twice(x) + 1.0;
  endfunction
  function automatic void swap(ref real p, ref real q);
    real t;
    t = p;
    p = q;
    q = t;
  endfunction
  // Every activation binds its own reference; the shared accumulator sees
  // each partial write immediately.
  task automatic sum(ref real x, input int k);
    real mine;
    mine = real'(k);
    if (k > 0) sum(x, k - 1);
    x = x + mine * 0.5;
  endtask
  task automatic slow(ref real x, input real step);
    repeat (3) #1 x = x + step;
  endtask

  always @(a[1]) hits = hits + 1;

  initial begin
    r = 1.25;
    add(r, 0.5);
    $display("r %.2f", r);
    s = shortreal'(0.1);
    triple(s);
    $display("s %h", $shortrealtobits(s));
    a[0] = 1.0; a[1] = 2.0; a[2] = 3.0;
    for (int i = 0; i < 3; i++) forward(a[i]);
    $display("a %.2f %.2f %.2f", a[0], a[1], a[2]);
    $display("look %.2f", look(a[2]));
    foreach (m[i, j]) m[i][j] = real'(i * 10 + j);
    idx = 1;
    add(m[idx][2], 0.5);
    $display("m %.1f", m[1][2]);
    idx = 7;
    add(a[idx], 100.0);
    $display("invalid %.2f %.2f %.2f", a[0], a[1], a[2]);
    rec.r = 0.5; rec.k = 3; rec.s = shortreal'(1.0);
    add(rec.r, 1.0);
    triple(rec.s);
    $display("rec %.2f %0d %h", rec.r, rec.k, $shortrealtobits(rec.s));
    lo = 1.0; hi = 2.0;
    swap(lo, hi);
    $display("swap %.1f %.1f", lo, hi);
    swap(lo, lo);
    $display("self %.1f", lo);
    acc = 0.0;
    sum(acc, 4);
    $display("sum %.2f", acc);
    #1;
    hits = 0;
    a[1] = 0.0;
    #1;
    fork
      slow(r, 0.5);
      slow(a[1], -1.5);
    join
    #1;
    $display("timed %.2f %.2f hits %0d", r, a[1], hits);
    $finish(0);
  end
endmodule
