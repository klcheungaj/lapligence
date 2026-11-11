// SIM-040: a dynamic array is the actual of an open-array formal (SV 7.7,
// 35.6.1): the formal has the size the array has at the call, with the range
// [0:size-1] (20.7), and is a copy, so an exported function that resizes the
// actual during the call changes neither the handle nor what is copied back.
// Build dynamic_actuals.c into a shared library and pass it with --dpi-lib.
module tb;
    typedef struct { int a; logic [3:0] t; } packed_rec_t;
    typedef struct { real r; string s; } native_rec_t;
    class holder;
        int values [];
    endclass

    import "DPI-C" function int da_sum(input string tag, input int a []);
    import "DPI-C" pure function int da_pure(input int a []);
    import "DPI-C" function void da_add(inout int a [], input int by);
    import "DPI-C" context function void da_resize(inout int a []);
    import "DPI-C" function void da_logic(inout logic [7:0] a []);
    import "DPI-C" function void da_packed(input string tag, input bit [] a []);
    import "DPI-C" function void da_reals(inout real a [], input shortreal by []);
    import "DPI-C" function void da_strings(inout string a []);
    import "DPI-C" function void da_packed_recs(inout packed_rec_t a []);
    import "DPI-C" function void da_native_recs(inout native_rec_t a []);
    export "DPI-C" function da_grow;

    int d [];
    int empty [];
    int fixed [4:2];
    logic [7:0] dl [];
    bit [11:0] dp [];
    bit [4:0] narrow [2];
    real dr [];
    shortreal ds [];
    string dn [];
    packed_rec_t dpr [];
    native_rec_t dnr [];
    holder h;

    function void da_grow(input int size);
        $display("export sees d=%p", d);
        d = new[size](d);
        d[size - 1] = 77;
    endfunction

    function automatic int total(input int a []);
        return da_sum("formal", a);
    endfunction

    initial begin
        d = new[3];
        d[0] = 1; d[1] = 2; d[2] = 3;
        $display("sum d=%0d", da_sum("d", d));
        $display("sum empty=%0d", da_sum("empty", empty));
        fixed[4] = 40; fixed[3] = 30; fixed[2] = 20;
        $display("sum fixed=%0d", da_sum("fixed", fixed));
        h = new;
        h.values = new[1];
        h.values[0] = 9;
        $display("sum property=%0d", da_sum("property", h.values));
        $display("sum formal=%0d", total(d));
        $display("pure=%0d", da_pure(d) + da_pure(d));

        da_add(d, 100);
        $display("d=%p", d);
        da_add(empty, 100);
        $display("empty size=%0d", empty.size());

        da_resize(d);
        $display("d=%p", d);

        dl = new[2];
        dl[0] = 8'h1x; dl[1] = 8'hzz;
        da_logic(dl);
        $display("dl=%h %h", dl[0], dl[1]);

        dp = new[2];
        dp[0] = 12'habc; dp[1] = 12'h123;
        da_packed("dp", dp);
        narrow[0] = 5'h1f; narrow[1] = 5'h03;
        da_packed("narrow", narrow);

        dr = new[3];
        dr[0] = 1.5; dr[1] = 2.5; dr[2] = 3.5;
        ds = new[3];
        ds[0] = 2.0; ds[1] = 0.5; ds[2] = 4.0;
        da_reals(dr, ds);
        $display("dr=%g %g %g", dr[0], dr[1], dr[2]);

        dn = new[3];
        dn[0] = "a"; dn[1] = "bb"; dn[2] = "ccc";
        da_strings(dn);
        $display("dn=%s %s %s", dn[0], dn[1], dn[2]);

        dpr = new[2];
        dpr[0].a = 5; dpr[0].t = 4'b1x0z; dpr[1].a = 6; dpr[1].t = 4'h3;
        da_packed_recs(dpr);
        $display("dpr=%0d %b %0d %b", dpr[0].a, dpr[0].t, dpr[1].a, dpr[1].t);

        dnr = new[2];
        dnr[0].r = 1.5; dnr[0].s = "n0"; dnr[1].r = 2.5; dnr[1].s = "n1";
        da_native_recs(dnr);
        $display("dnr=%g %s %g %s", dnr[0].r, dnr[0].s, dnr[1].r, dnr[1].s);

        $finish;
    end
endmodule
