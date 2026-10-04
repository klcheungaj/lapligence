// SIM-004: conditional operators with string and native record results.
// IEEE 1800-2009 11.4.11: a known predicate evaluates one arm; an ambiguous
// one evaluates both and combines them element by element, keeping matching
// elements and giving others their default-uninitialized value (6.8 Table
// 6-7: "" for strings, null for chandles, X/0 for 4/2-state integral, 0.0
// for real). Nested records are immediate members and default as a whole.
module tb;
  typedef struct {string u; int k;} inner_t;
  typedef struct {
    string s;
    int n;
    chandle h;
    logic [3:0] v;
    inner_t in;
    real r;
  } rec_t;
  string a = "aa", b = "bb", c;
  logic x;
  int calls;
  rec_t ra, rb, rc;

  function automatic string twice(string v);
    calls++;
    return {v, v};
  endfunction

  function automatic rec_t mk(string s, int n);
    rec_t r;
    calls++;
    r.s = s;
    r.n = n;
    r.v = 4'b0101;
    return r;
  endfunction

  function automatic rec_t pick(input logic sel, input rec_t p, q);
    return sel ? p : q;
  endfunction

  initial begin
    x = 1'bx;
    c = x ? a : b;
    $display("1 [%s]", c);
    c = x ? a : a;
    $display("2 [%s]", c);
    x = 1;
    c = x ? a : b;
    $display("3 [%s]", c);
    x = 0;
    c = x ? a : b;
    $display("4 [%s]", c);
    calls = 0;
    x = 1'bz;
    c = x ? twice(a) : twice(b);
    $display("5 [%s] %0d", c, calls);
    x = 1;
    c = x ? twice(a) : twice(b);
    $display("6 [%s] %0d", c, calls);
    x = 1'bx;
    c = x ? twice(a) : {a, a};
    $display("7 [%s] %0d", c, calls);
    $display("8 [%s] [%s]", x ? a : b, x ? b : b);

    ra = '{"p", 1, null, 4'b10x1, '{"u", 1}, 1.5};
    rb = '{"q", 1, null, 4'b10x1, '{"u", 2}, 1.5};
    rc = x ? ra : rb;
    $display("9 [%s] %0d %0d %b [%s] %0d %0.1f", rc.s, rc.n, rc.h == null, rc.v, rc.in.u,
             rc.in.k, rc.r);
    x = 0;
    rc = x ? ra : rb;
    $display("10 [%s] %0d [%s] %0d", rc.s, rc.n, rc.in.u, rc.in.k);
    x = 1'bx;
    rc = x ? mk("m", 5) : mk("m", 6);
    $display("11 [%s] %0d %b %0d", rc.s, rc.n, rc.v, calls);
    x = 1;
    rc = x ? mk("k", 7) : mk("z", 8);
    $display("12 [%s] %0d %0d", rc.s, rc.n, calls);
    rc = pick(1'bx, ra, rb);
    $display("13 [%s] %0d", rc.s, rc.n);
    rc = pick(1'b1, ra, rb);
    $display("14 [%s] %0d", rc.s, rc.n);
    x = 1'bx;
    rc = x ? rc : ra;
    $display("15 [%s] %0d %b [%s] %0d", rc.s, rc.n, rc.v, rc.in.u, rc.in.k);
    rc.s = "old";
    rb.s = "p";
    rc <= x ? ra : rb;
    $display("16 [%s]", rc.s);
    #1 $display("17 [%s] %0d [%s] %0d", rc.s, rc.n, rc.in.u, rc.in.k);
    $finish(0);
  end
endmodule
