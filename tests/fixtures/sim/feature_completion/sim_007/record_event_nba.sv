`timescale 1ns/1ns
module tb;
  class C; int v; function new(int x); v = x; endfunction endclass
  typedef struct { string t; int y; } in_t;
  typedef struct { string s; int n; logic [7:0] b; real r; C h; int a[3]; string q[3]; in_t i; } el_t;
  typedef struct { string name; el_t e[3]; } rec_t;
  el_t nb[3];
  rec_t m, o;
  el_t src;
  string x = "x1";
  int k = 1, j = 2, y = 1, n = 0;
  event e;
  logic [7:0] bv = 8'h11;
  C h1;
  function automatic int f(int v);
    n++;
    $display("%0t f %0d", $time, v);
    return v;
  endfunction
  initial begin
    h1 = new(5);
    // (1) nb[k] with k at issue
    nb[k].s <= @(e) x;
    nb[k].n <= @(e) y;
    nb[k].b <= @(e) bv;
    nb[k].r <= @(e) 1.5;
    nb[k].h <= @(e) h1;
    nb[k].a[j] <= @(e) y;
    nb[k].q[j] <= @(e) x;
    m.e[k].s <= @(e) x;
    m.e[k].a[j] <= @(e) y;
    m.e[k].b[j] <= @(e) 1'b1;
    m.e[k].q[j] <= @(e) x;
    k = 2; j = 0; x = "x2"; y = 2; bv = 8'h22; h1 = new(6);
    #1 $display("%0t pre [%s] %0d", $time, nb[1].s, nb[1].n);
    -> e;
    $display("%0t same [%s]", $time, nb[1].s);
    #0 $display("%0t #0 [%s]", $time, nb[1].s);
    #1 $display("%0t [%s] %0d %h %0.1f %0d %0d [%s]", $time, nb[1].s, nb[1].n, nb[1].b, nb[1].r, nb[1].h.v, nb[1].a[2], nb[1].q[2]);
    $display("%0t [%s] %0d %b [%s]", $time, m.e[1].s, m.e[1].a[2], m.e[1].b[2], m.e[1].q[2]);
    // (2) repeat
    k = 0;
    x = "r";
    nb[k].s <= repeat (2) @(e) x;
    x = "z";
    #1 -> e;
    #1 $display("%0t [%s]", $time, nb[0].s);
    -> e;
    #1 $display("%0t [%s]", $time, nb[0].s);
    // (3) repeat 0: this time step
    nb[k].n <= repeat (0) @(e) 7;
    $display("%0t %0d", $time, nb[0].n);
    #0 $display("%0t %0d", $time, nb[0].n);
    #1 $display("%0t %0d", $time, nb[0].n);
    // (4) selector, value, count once and in order
    nb[f(1)].a[f(0)] <= repeat (f(1)) @(e) f(5);
    #1 -> e;
    #1 $display("%0t %0d", $time, nb[1].a[0]);
    // (5) a whole element write between issue and the event
    nb[1].s <= @(e) "kept";
    nb[1] = src;
    #1 $display("%0t [%s]", $time, nb[1].s);
    -> e;
    #1 $display("%0t [%s]", $time, nb[1].s);
    // (6) out of range index writes nothing
    k = 9;
    nb[k].s <= @(e) "no";
    m.e[k].s <= @(e) "no";
    m.e[1].a[k] <= @(e) 9;
    #1 -> e;
    #1 $display("%0t [%s] %0d", $time, m.e[1].s, m.e[1].a[0]);
    // (7) whole record
    o.name = "o1";
    o.e[1].s = "oe";
    m <= @(e) o;
    o.name = "o2";
    #1 $display("%0t [%s]", $time, m.name);
    -> e;
    #1 $display("%0t [%s] [%s]", $time, m.name, m.e[1].s);
    m <= repeat (2) @(e) o;
    #1 -> e;
    #1 -> e;
    #1 $display("%0t [%s]", $time, m.name);
    // (8) whole element, sub-record
    src.s = "src";
    src.i.t = "it";
    k = 1;
    nb[k] <= @(e) src;
    m.e[k].i <= @(e) src.i;
    nb[k].i <= @(e) src.i;
    src.s = "src2";
    src.i.t = "it2";
    k = 0;
    #1 -> e;
    #1 $display("%0t [%s] [%s] [%s]", $time, nb[1].s, m.e[1].i.t, nb[1].i.t);
    $finish(0);
  end
endmodule
