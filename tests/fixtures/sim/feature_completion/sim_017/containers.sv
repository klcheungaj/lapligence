// SIM-017: whole queues and dynamic arrays as messages, including a queue of
// strings and a queue of records. Each message is a deep copy taken at put:
// later changes to the source, or to a received copy, are independent
// (SV 15.4, 7.5, 7.10).
module tb;
  typedef int iq_t[$];
  typedef string sq_t[$];
  typedef logic [3:0] nib_t[];
  typedef struct {
    int k;
    string s;
  } item_t;
  typedef item_t rq_t[$];

  mailbox #(iq_t) ints = new(1);
  mailbox #(sq_t) strings = new();
  mailbox #(nib_t) nibbles = new();
  mailbox any = new();
  iq_t a, b;
  sq_t s, t;
  nib_t d, e;
  rq_t r, u;
  item_t it;

  initial begin
    a = '{1, 2, 3};
    ints.put(a);
    a.push_back(4);
    a[0] = 100;
    $display("full try_put=%0d n=%0d", ints.try_put(a), ints.num());
    ints.get(b);
    $display("ints %0d %0d %0d size=%0d", b[0], b[1], b[2], b.size());
    b[1] = 50;
    $display("source %0d %0d size=%0d", a[0], a[1], a.size());

    s = '{"x", "yy"};
    strings.put(s);
    s[0] = "zz";
    strings.peek(t);
    $display("peek %s %s n=%0d", t[0], t[1], strings.num());
    t[1] = "changed";
    strings.get(t);
    $display("get %s %s n=%0d", t[0], t[1], strings.num());

    d = new[3];
    d[0] = 4'h1;
    d[1] = 4'hx;
    d[2] = 4'hf;
    nibbles.put(d);
    d = new[1];
    nibbles.get(e);
    $display("dyn size=%0d %b %b %b", e.size(), e[0], e[1], e[2]);

    it.k = 7;
    it.s = "seven";
    r.push_back(it);
    it.k = 8;
    it.s = "eight";
    r.push_back(it);
    any.put(r);
    r[0].s = "mutated";
    r.delete();
    any.get(u);
    $display("records size=%0d %0d %s %0d %s", u.size(), u[0].k, u[0].s, u[1].k,
             u[1].s);
    any.put(b);
    $display("as_strings=%0d n=%0d", any.try_get(t), any.num());
    $display("as_ints=%0d size=%0d n=%0d", any.try_get(a), a.size(), any.num());
    $display("a %0d %0d %0d", a[0], a[1], a[2]);
    $finish;
  end
endmodule
