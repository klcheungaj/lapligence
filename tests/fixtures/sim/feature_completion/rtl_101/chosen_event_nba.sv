// Whole records, elements and members with column storage written through an
// event control or a delay: value and selectors at issue, write after the
// control. The same shapes below the column size stay per part.
typedef struct packed { logic [3:0] a; logic [3:0] b; } pair_t;
typedef struct { int i; int a[16]; logic [7:0] b[16]; real d[16]; } el_t;
typedef struct { el_t e[2]; int x; real r[16]; } top_t;
typedef struct { pair_t p[16]; shortreal sr[16]; bit [1:0] m[16][2]; } mix_t;
typedef struct { int a[4]; real d[4]; } small_t;

module tb;
  el_t arr[3], src;
  top_t m, o;
  mix_t x, y;
  small_t part[2], ps;
  event e;
  int k;

  function automatic int f(int v);
    $display("f%0d", v);
    return v;
  endfunction

  initial begin
    // (1) an element: selector and value at issue
    src.i = 3;
    src.a[2] = 22;
    src.b[2] = 8'h5a;
    src.d[2] = 2.5;
    k = 1;
    arr[k] <= @(e) src;
    k = 0;
    src.a[2] = 0;
    src.b[2] = 0;
    src.d[2] = 9.0;
    src.i = 0;
    #1 $display("%0t %0d %0d %0h %0.2f", $time, arr[1].i, arr[1].a[2], arr[1].b[2], arr[1].d[2]);
    -> e;
    #1 $display("%0t %0d %0d %0h %0.2f", $time, arr[1].i, arr[1].a[2], arr[1].b[2], arr[1].d[2]);
    // (2) a whole record
    o.e[1].i = 7;
    o.e[1].a[3] = 33;
    o.e[1].d[3] = 3.5;
    o.r[1] = 1.25;
    o.x = 1;
    m <= @(e) o;
    o.e[1].i = 0;
    o.e[1].a[3] = 0;
    o.e[1].d[3] = 0;
    o.r[1] = 0;
    o.x = 0;
    #1 $display("%0t %0d %0d %0.2f %0.2f %0d", $time, m.e[1].i, m.e[1].a[3], m.e[1].d[3], m.r[1], m.x);
    -> e;
    #1 $display("%0t %0d %0d %0.2f %0.2f %0d", $time, m.e[1].i, m.e[1].a[3], m.e[1].d[3], m.r[1], m.x);
    // (3) repeat count once, after the value
    o.e[1].a[3] = 34;
    m <= repeat (f(2)) @(e) o;
    #1 -> e;
    #1 $display("%0t %0d", $time, m.e[1].a[3]);
    -> e;
    #1 $display("%0t %0d", $time, m.e[1].a[3]);
    // (4) a delay
    o.r[1] = 4.5;
    o.e[1].a[3] = 35;
    o.e[1].d[3] = 8.5;
    m <= #2 o;
    o.r[1] = 0;
    o.e[1].a[3] = 0;
    o.e[1].d[3] = 0;
    #1 $display("%0t %0d %0.2f %0.2f", $time, m.e[1].a[3], m.e[1].d[3], m.r[1]);
    #2 $display("%0t %0d %0.2f %0.2f", $time, m.e[1].a[3], m.e[1].d[3], m.r[1]);
    // (5) packed, short real and multidimensional members
    y.p[2] = 8'h3c;
    y.sr[2] = 1.5;
    y.m[2][1] = 2;
    x <= @(e) y;
    y.p[2] = 0;
    y.sr[2] = 0;
    y.m[2][1] = 0;
    #1 $display("%0t %0h %0.2f %0d", $time, x.p[2], x.sr[2], x.m[2][1]);
    -> e;
    #1 $display("%0t %0h %0.2f %0d", $time, x.p[2], x.sr[2], x.m[2][1]);
    y.p[2] = 8'hc3;
    y.sr[2] = 2.5;
    y.m[2][1] = 1;
    x <= #1 y;
    y.p[2] = 0;
    y.sr[2] = 0;
    y.m[2][1] = 0;
    #2 $display("%0t %0h %0.2f %0d", $time, x.p[2], x.sr[2], x.m[2][1]);
    // (6) an element of a member, a part of a member
    src.a[2] = 44;
    src.d[2] = 4.25;
    k = 1;
    m.e[k] <= @(e) src;
    k = 0;
    src.a[2] = 0;
    src.d[2] = 0;
    #1 $display("%0t %0d %0.2f", $time, m.e[1].a[2], m.e[1].d[2]);
    -> e;
    #1 $display("%0t %0d %0.2f", $time, m.e[1].a[2], m.e[1].d[2]);
    k = 2;
    arr[k].d[k] <= @(e) 6.5;
    k = 0;
    #1 -> e;
    #1 $display("%0t %0.2f", $time, arr[2].d[2]);
    // (7) out of range selects write nothing
    k = 9;
    arr[k] <= @(e) src;
    m.e[k] <= @(e) src;
    #1 -> e;
    #1 $display("%0t %0d", $time, arr[1].a[2]);
    // (8) per-part members
    ps.a[1] = 5;
    ps.d[1] = 5.5;
    k = 1;
    part[k] <= @(e) ps;
    k = 0;
    ps.a[1] = 0;
    ps.d[1] = 0;
    #1 $display("%0t %0d %0.2f", $time, part[1].a[1], part[1].d[1]);
    -> e;
    #1 $display("%0t %0d %0.2f", $time, part[1].a[1], part[1].d[1]);
    ps.a[1] = 6;
    ps.d[1] = 6.5;
    part[0] <= #1 ps;
    ps.a[1] = 0;
    ps.d[1] = 0;
    #2 $display("%0t %0d %0.2f", $time, part[0].a[1], part[0].d[1]);
    // (9) never fired
    m <= @(e) o;
    x <= repeat (3) @(e) y;
    $finish(0);
  end
endmodule
