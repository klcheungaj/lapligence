// Records whose member arrays have no packed column, in every declaration
// storage and at the sizes that change their form.
module tb;
  typedef struct { string a[16]; int tag; } strs_t;
  typedef struct { int a[16]; string s; } ints_t;
  typedef struct { string d[5000]; int n; } large_strs_t;
  typedef struct { real d[5000]; string t; } large_reals_t;
  typedef struct { int p[5000]; string d[20]; } mixed_strs_t;
  typedef struct { int p[5000]; real d[20]; string t; } mixed_reals_t;
  typedef struct { real d[4095]; string t; } column_reals_t;
  typedef struct { real d[2][8]; string t; } grid_t;

  class shared_t;
    static ints_t x;
    static function void set(int v); x.a[9] = v; x.s = "c"; endfunction
  endclass

  large_strs_t ls;
  large_reals_t lr, lq;
  mixed_strs_t ms, mt;
  mixed_reals_t mr;
  column_reals_t cr, cq;
  grid_t g;

  initial begin
    // A static block record has one set of columns, an automatic one starts
    // from defaults at every entry, with its declared size.
    for (int i = 0; i < 2; i++) begin
      strs_t st;
      automatic strs_t au;
      ints_t si;
      $display("entry %0d [%s] [%s] %0d", i, st.a[1], au.a[1], si.a[3]);
      st.a[1] = "kept"; au.a[1] = "lost"; si.a[3] = 40 + i; si.s = "s";
      $display("set [%s] [%s] %0d [%s]", st.a[1], au.a[1], si.a[3], si.s);
    end
    fork
      begin
        automatic strs_t f;
        f.a[15] = "forked";
        #1 $display("[%s] %0d", f.a[15], $size(f.a));
      end
    join

    // Above the dense threshold the cells live in a container.
    ls.d[4999] = "last"; ls.n = 1;
    $display("[%s] %0d", ls.d[4999], $size(ls.d));
    lr.d[4999] = 2.5; lq = lr;
    $display("%f %0d", lq.d[4999], lq == lr);
    lq.d[0] = 1.0;
    $display("%0d", lq == lr);

    // A packed member beside a native array follows the record.
    ms.p[4999] = 7; ms.d[19] = "z"; mt = ms;
    $display("%0d [%s] %0d", mt.p[4999], mt.d[19], mt == ms);
    mr.p[2] = 3; mr.d[19] = 0.5;
    $display("%0d %f", mr.p[2], mr.d[19]);

    // A whole real column compares once, by value or by bit pattern.
    cr.d[4094] = 0.0; cq = cr; cq.d[4094] = -0.0;
    $display("%0d %0d", cr.d == cq.d, cr.d === cq.d);
    cq.d[0] = 1.0;
    $display("%0d %0d", cr.d == cq.d, cr.d !== cq.d);

    shared_t::set(5);
    $display("%0d [%s]", shared_t::x.a[9], shared_t::x.s);

    // Part of a column moves cell by cell.
    g.d[0][3] = 1.5; g.d[1][3] = 2.5;
    g.d[1] = g.d[0];
    $display("%f %f", g.d[1][3], g.d[0][3]);
    $finish(0);
  end
endmodule
