import "DPI-C" function chandle mk(int i);
module tb;
  typedef int table_t[chandle];
  table_t a, b;
  string s[chandle];
  int q[string][chandle];
  class C;
    int m[chandle];
  endclass
  C o = new;
  chandle c0, c1, c2, k;
  initial begin
    c0 = mk(0); c1 = mk(1); c2 = mk(2);
    a[c1] = 11; a[c0] = 10; a[null] = 5;
    $display("%0d %0d %0d %0d", a[c1], a[c0], a[null], a.num());
    $display("%0d %0d", a.exists(c2), a.exists(c0));
    s[c1] = "one";
    $display("%s <%s>", s[c1], s[c2]);
    a[c1]++; a[c2] += 3;
    $display("%0d %0d", a[c1], a[c2]);
    foreach (a[i]) $display("%b %b %b %b", i == null, i == c0, i == c1, i == c2);
    if (a.first(k)) begin
      $display("first null %b", k == null);
      while (a.next(k)) $display("n %0d", a[k]);
    end
    a.delete(c1);
    $display("%0d %0d", a.num(), a.exists(c1));
    a.delete();
    $display("%0d", a.num());
    o.m[c2] = 7; o.m[null] = 8;
    foreach (o.m[i]) $display("m %b %0d", i == c2, o.m[i]);
    q["x"][c0] = 4; q["x"][null] = 3;
    $display("%0d %0d", q["x"].num(), q["x"][c0]);
    a[null] = 1;
    $display("%p", a);
    a[c0] = 6; b = a;
    $display("%0d %0d", b.num(), b[c0]);
    $display("%0d %0d", a.last(k), k == c0);
    $display("%0d %0d", a.prev(k), k == null);
    $display("%0d %0d", a.prev(k), k == null);
    s = '{null: "n", default: "d"};
    $display("%s %s", s[c0], s[null]);
    $display("%0d %0d", a.sum(), a.sum() with (item * 2));
    $finish(0);
  end
endmodule
