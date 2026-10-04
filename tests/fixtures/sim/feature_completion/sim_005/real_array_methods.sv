// SIM-005: ordering and locator methods over real elements compare numbers,
// never bit patterns (IEEE 1800-2009 7.12.1, 7.12.2).
module tb;
  real a[5], m[2][2], q[$], r[$], d[], zero, nan;
  shortreal s[3];
  int idx[$], changes;

  always @(a[0]) changes = changes + 1;

  initial begin
    zero = 0.0;
    nan = zero / zero;
    a = '{1.5, -2.0, 4.25, 0.5, -2.0};
    #1;
    changes = 0;
    a.sort();
    $display("sort %.2f %.2f %.2f %.2f %.2f", a[0], a[1], a[2], a[3], a[4]);
    a.rsort();
    $display("rsort %.2f %.2f %.2f %.2f %.2f", a[0], a[1], a[2], a[3], a[4]);
    a.reverse();
    $display("reverse %.2f %.2f %.2f %.2f %.2f", a[0], a[1], a[2], a[3], a[4]);
    #1;
    $display("changes %0d", changes);
    s = '{shortreal'(0.3), shortreal'(0.1), shortreal'(0.2)};
    s.sort();
    $display("short %h %h %h", $shortrealtobits(s[0]), $shortrealtobits(s[1]),
             $shortrealtobits(s[2]));
    m = '{'{1.0, 2.0}, '{3.0, 4.0}};
    m.reverse();
    m[1].reverse();
    $display("rows %.1f %.1f %.1f %.1f", m[0][0], m[0][1], m[1][0], m[1][1]);
    // A NaN key keeps its position; the keys on each side sort separately.
    a = '{3.0, 1.0, 0.0, 2.0, -1.0};
    a[2] = nan;
    a.sort();
    $display("nan %.1f %.1f %0d %.1f %.1f", a[0], a[1], a[2] != a[2], a[3], a[4]);
    q = '{3.0, 1.0, 2.0, 1.0, -0.0, 0.0};
    r = q.find(x) with (x > 1.5);
    $display("find %0d %.1f %.1f", r.size(), r[0], r[1]);
    idx = q.find_index(x) with (x < 2.5);
    $display("find_index %0d %0d %0d", idx.size(), idx[0], idx[idx.size() - 1]);
    r = q.find_first(x) with (x < 2.0);
    idx = q.find_last_index(x) with (x == 1.0);
    $display("first %.1f last_index %0d", r[0], idx[0]);
    r = q.min();
    $display("min %0d %h", r.size(), $realtobits(r[0]));
    r = q.max();
    $display("max %.1f", r[0]);
    r = q.unique();
    idx = q.unique_index();
    $display("unique %0d %0d %0d %0d %0d", r.size(), idx[0], idx[1], idx[2], idx[3]);
    q.sort();
    $display("qsort %.1f %.1f %.1f %.1f", q[0], q[1], q[4], q[5]);
    q.rsort();
    q.reverse();
    $display("qreverse %.1f %.1f", q[0], q[5]);
    d = new[4];
    d[0] = 0.5; d[1] = -1.0; d[2] = 0.25; d[3] = 2.0;
    d.sort();
    $display("dsort %.2f %.2f %.2f %.2f", d[0], d[1], d[2], d[3]);
    r = d.find(e) with (e > 0.0 && e.index != 1);
    $display("dfind %0d %.2f", r.size(), r[0]);
    q = '{nan, 2.0, nan, -3.0};
    r = q.min();
    $display("nanmin %.1f", r[0]);
    r = q.unique();
    $display("nanunique %0d", r.size());
    q.delete();
    r = q.max();
    $display("empty %0d", r.size());
    $finish(0);
  end
endmodule
