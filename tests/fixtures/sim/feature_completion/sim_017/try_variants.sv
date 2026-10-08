// SIM-017: try_put/try_get/try_peek/num of record and scalar messages: empty
// (0), full (0), success (positive) and type mismatch (negative). A failed
// call changes neither the mailbox nor the destination; peek does not
// consume (SV 15.4.3-15.4.8, 6.22).
module tb;
  typedef struct {
    int a;
    string s;
  } r_t;
  typedef struct {
    int a;
    string s;
  } other_t;

  mailbox #(r_t) b = new(2);
  mailbox u = new();
  r_t x, y, keep;
  other_t o;
  int i;
  string s;
  real r;
  logic [7:0] l;
  byte by;

  initial begin
    y.a = -1;
    y.s = "dest";
    $display("empty get=%0d peek=%0d y=%0d %s n=%0d", b.try_get(y), b.try_peek(y), y.a,
             y.s, b.num());
    x.a = 1;
    x.s = "one";
    $display("put %0d %0d %0d n=%0d", b.try_put(x), b.try_put(x), b.try_put(x), b.num());
    x.a = 2;
    x.s = "two";
    $display("peek=%0d y=%0d %s n=%0d", b.try_peek(y), y.a, y.s, b.num());
    y.s = "local";
    $display("get=%0d y=%0d %s n=%0d", b.try_get(y), y.a, y.s, b.num());
    b.put(x);
    $display("full=%0d n=%0d", b.try_put(x), b.num());
    b.get(y);
    b.get(y);
    $display("fifo %0d %s n=%0d", y.a, y.s, b.num());
    u.put(x);
    o.a = 9;
    o.s = "nine";
    i = 5;
    s = "str";
    r = 1.5;
    $display("mismatch %0d %0d %0d %0d n=%0d", u.try_get(o), u.try_peek(i), u.try_get(s),
             u.try_get(r), u.num());
    $display("kept %0d %s %0d %s %0.1f", o.a, o.s, i, s, r);
    $display("match %0d %0d %s n=%0d", u.try_get(keep), keep.a, keep.s, u.num());
    l = 8'hx5;
    u.put(l);
    l = 8'h00;
    $display("byte=%0d by=%0d logic=%0d l=%h", u.try_get(by), by, u.try_peek(l), l);
    l = 8'h11;
    $display("int=%0d i=%0d l=%h n=%0d", u.try_get(i), i, l, u.num());
    $display("final=%0d l=%h n=%0d", u.try_get(l), l, u.num());
    $finish;
  end
endmodule
