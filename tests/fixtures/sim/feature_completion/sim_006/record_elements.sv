// SIM-006 A01: unpacked records with string, real, nested record and fixed
// array members as elements of queues, dynamic and associative arrays
// (SV 7.4, 7.5, 7.8, 7.10). Elements are deep copies; class-handle members
// would copy the handle only.
module tb;
  typedef struct { int a; logic [3:0] n; } in_t;
  typedef struct { string name; in_t inner; int arr[2]; real r; } rec_t;
  rec_t q[$];
  rec_t d[];
  rec_t m[string];
  rec_t w[int];
  rec_t x, y;
  int i;
  initial begin
    x.name = "first";
    x.inner.a = 3;
    x.inner.n = 4'hA;
    x.arr[0] = 10;
    x.arr[1] = 11;
    x.r = 2.5;
    q.push_back(x);
    x.name = "second";
    x.arr[1] = 21;
    q.push_front(x);
    q.insert(1, x);
    $display("size %0d %s %s %s", q.size(), q[0].name, q[1].name, q[2].name);
    i = 2;
    q[i].arr[1] = 99;
    q[i].inner.n = 4'h5;
    q[i].name = {q[i].name, "!"};
    $display("%0d %h %s %0d %0d", q[2].arr[1], q[2].inner.n, q[2].name, q[2].inner.a,
             q[0].arr[1]);
    y = q[2];
    q[2].name = "changed";
    $display("copy %s %s %0d", y.name, q[2].name, y.arr[1]);
    m["k"] = x;
    m["k"].name = "keyed";
    w[7].name = "created";
    $display("assoc %s %0d %s %0d %0d", m["k"].name, m.num(), w[7].name, w.exists(7),
             w[7].arr[0]);
    y = q.pop_back();
    $display("pop %s %0d %.2f", y.name, q.size(), y.r);
    q.delete(0);
    $display("left %s %0d", q[0].name, q.size());
    d = new[2];
    d[0] = x;
    d[1].name = "z";
    d = new[3](d);
    d[2].r = 4.25;
    $display("dyn %0d %s %s [%s] %.2f %0d", d.size(), d[0].name, d[1].name, d[2].name,
             d[2].r, d[0].inner.a);
    d.delete();
    q.delete();
    $display("empty %0d %0d", d.size(), q.size());
    $finish(0);
  end
endmodule
