// Member arrays of column records: a member array as an `inside` set (SV
// 11.4.13), a `case inside` item, element patterns, fills of structured
// elements, and nested records. `-D N=8` prints the same lines per cell.
`ifndef N
`define N 16
`endif
module tb;
  typedef struct { byte a; byte b; } pair_t;
  typedef struct { pair_t p [`N]; byte t; } rec_t;
  typedef struct { byte a [`N]; byte t; } arr_t;
  typedef struct { rec_t r; int k; } outer_t;
  rec_t r, s;
  arr_t x, y, z;
  outer_t o, w;
  pair_t q;
  arr_t list [3];
  int i;
  function automatic rec_t pass(rec_t v);
    return v;
  endfunction
  initial begin
    i = 3;
    x.a[i] = 5; x.t = 2;
    y = x;
    z = y; z.a[4] = 1;
    $display("in %b %b", 8'd5 inside {x.a}, 8'd1 inside {x.a, y.a});
    list[1] = x;
    $display("in list %b %b", 8'd5 inside {list[1].a}, 8'd9 inside {list[i - 2].a});
    r.p[i].a = 7;
    r.p[2].b = 8;
    s = r;
    $display("in rec %b %b", r.p[i].a inside {s.p[3].a, s.p[2].a}, s.p[2].b inside {r.p[i].a, 4});
    q = '{7, 0};
    s.p[1] = '{1, 2};
    $display("pat %0d %0d", s.p[1].a, s.p[1].b);
    r = '{p: '{default: pair_t'{a: 3, b: 4}}, t: 9};
    $display("default %0d %0d %0d", r.p[`N - 1].a, r.p[0].b, r.t);
    r = '{default: 0};
    $display("zero %0d", r.p[2].b);
    o.r.p[i].b = 6;
    w = o;
    $display("nest %b %0d", w == o, w.r.p[3].b);
    w.r = pass(o.r);
    w.r.p[3] = '{1, 1};
    $display("nest %b %0d", w == o, w.r.p[3].b);
    case (x.a[i]) inside
      y.a: $display("case in");
      default: $display("case none");
    endcase
    casez (i) 3: $display("case"); endcase
    foreach (r.p[k]) r.p[k].a = 8'(k);
    $display("foreach %0d", r.p[5].a);
    $finish;
  end
endmodule
