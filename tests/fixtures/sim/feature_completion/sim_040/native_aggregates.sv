// SIM-040: unpacked structures and sized unpacked arrays whose leaves are
// real, shortreal, string or chandle values cross the DPI in the C compiler's
// layout (SV H.7.8, Table H.1), strings as const char* (H.8.10.1), lowest
// index first (H.7.3), in every direction. Build native_aggregates.c into a
// shared library and pass it with --dpi-lib.
module tb;
    typedef struct { real r; int i; string s; chandle h; shortreal f; byte b; } leaf_t;
    typedef struct { int a; shortint q [2]; } plain_t;
    typedef struct {
        leaf_t one;
        real pair [1:0];
        string names [2];
        logic [3:0] tag;
        plain_t plain;
    } nest_t;
    typedef string row_t [3];
    typedef struct { string s; int big [20]; } wide_t;

    import "DPI-C" function chandle na_handle(input int id);
    import "DPI-C" function int na_id(input chandle h);
    import "DPI-C" function void na_leaf(input leaf_t i, output leaf_t o, inout leaf_t io);
    import "DPI-C" function void na_nest(input nest_t i, inout nest_t io);
    import "DPI-C" function void na_strings(input string a [2], output string o [3:2],
        inout string io [2][2], input row_t rows [1:0]);
    import "DPI-C" function void na_reals(input real a [3:1], output real o [2][2],
        inout shortreal io [0:1]);
    import "DPI-C" function void na_handles(input chandle a [2], output chandle o [1:0],
        inout chandle io [2]);
    import "DPI-C" function void na_records(input leaf_t a [2:1], output leaf_t o [2]);
    import "DPI-C" function void na_order(input int k, output leaf_t first, output int n,
        output leaf_t second);
    import "DPI-C" task na_task(input string a [2], output real o [2], inout leaf_t io);
    import "DPI-C" function void na_wide(inout wide_t a [2], input wide_t copy []);

    leaf_t li, lo, lio;
    nest_t ni, nio;
    string sa [2];
    string so [3:2];
    string sio [2][2];
    row_t rows [1:0];
    real ra [3:1];
    real ro [2][2];
    shortreal fio [0:1];
    chandle ha [2];
    chandle ho [1:0];
    chandle hio [2];
    leaf_t la [2:1];
    leaf_t lout [2];
    real tout [2];
    wide_t wa [2];
    int n;

    function automatic void show(input string tag, input leaf_t v);
        $display("%s r=%g i=%0d s=%s h=%0d f=%g b=%0d", tag, v.r, v.i, v.s, na_id(v.h), v.f, v.b);
    endfunction

    initial begin
        li.r = 1.5; li.i = -7; li.s = "in"; li.h = na_handle(11); li.f = 2.25; li.b = 8'sd9;
        lo.r = 9.0; lo.i = 9; lo.s = "old"; lo.h = na_handle(99); lo.f = 9.0; lo.b = 8'sd9;
        lio.r = -3.25; lio.i = 42; lio.s = "io"; lio.h = na_handle(22); lio.f = 0.5;
        lio.b = -8'sd3;
        na_leaf(li, lo, lio);
        show("lo", lo);
        show("lio", lio);

        ni.one = li; ni.pair[1] = 10.5; ni.pair[0] = 20.5; ni.names[0] = "n0";
        ni.names[1] = "n1"; ni.tag = 4'b1x0z; ni.plain.a = 5; ni.plain.q[0] = -16'sd6;
        ni.plain.q[1] = 16'sd7;
        nio = ni; nio.one.s = "nio";
        na_nest(ni, nio);
        show("nio.one", nio.one);
        $display("nio pair=%g %g names=%s %s tag=%b plain=%0d %0d %0d", nio.pair[1], nio.pair[0],
            nio.names[0], nio.names[1], nio.tag, nio.plain.a, nio.plain.q[0], nio.plain.q[1]);

        sa[0] = "s0"; sa[1] = "s1"; so[3] = "old3"; so[2] = "old2";
        sio[0][0] = "a"; sio[0][1] = "b"; sio[1][0] = "c"; sio[1][1] = "d";
        rows[1][0] = "r10"; rows[1][1] = "r11"; rows[1][2] = "r12";
        rows[0][0] = "r00"; rows[0][1] = "r01"; rows[0][2] = "r02";
        na_strings(sa, so, sio, rows);
        $display("so[3]=%s so[2]=%s sio=%s %s %s %s", so[3], so[2], sio[0][0], sio[0][1],
            sio[1][0], sio[1][1]);

        ra[3] = 3.5; ra[2] = 2.5; ra[1] = 1.5; fio[0] = 0.5; fio[1] = 1.5;
        ro[0][0] = 9.0; ro[0][1] = 9.0; ro[1][0] = 9.0; ro[1][1] = 9.0;
        na_reals(ra, ro, fio);
        $display("ro=%g %g %g %g fio=%g %g", ro[0][0], ro[0][1], ro[1][0], ro[1][1], fio[0],
            fio[1]);

        ha[0] = na_handle(1); ha[1] = na_handle(2); ho[1] = na_handle(9);
        hio[0] = na_handle(3); hio[1] = null;
        na_handles(ha, ho, hio);
        $display("ho[1]=%0d ho[0]=%0d hio=%0d %0d", na_id(ho[1]), na_id(ho[0]), na_id(hio[0]),
            na_id(hio[1]));

        la[2] = li; la[1] = lio; la[1].s = "first";
        na_records(la, lout);
        show("lout[0]", lout[0]);
        show("lout[1]", lout[1]);

        // Outputs are copied out in declaration order: the last one written
        // to an actual named twice stays (decision S40-D2).
        na_order(3, lo, n, lo);
        show("order", lo);
        $display("n=%0d", n);

        na_task(sa, tout, lio);
        $display("task tout=%g %g", tout[0], tout[1]);
        show("task lio", lio);

        // An array of structures with a 20-cell member array: each element
        // is one value of the array's storage, whatever its size.
        wa[0].s = "w0"; wa[1].s = "w1";
        for (int k = 0; k < 20; k++) begin
            wa[0].big[k] = k;
            wa[1].big[k] = 100 + k;
        end
        na_wide(wa, wa);
        $display("wa=%s %0d %0d %s %0d", wa[0].s, wa[0].big[0], wa[0].big[19], wa[1].s,
            wa[1].big[19]);
        $finish;
    end
endmodule
