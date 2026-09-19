// llg-test-fixture: G1-34 rtl_no_silent_omissions (defect witness, ignored).
// IEEE 1800-2009 7.2.1/10.9: a packed-structure value may be assigned to a
// packed-structure member of an unpacked structure. The composed assignment
// must copy the packed value into that member. The current lowerer reports
// `unpacked aggregate used as a scalar assignment LHS` (expressions/aggregates.rs,
// collection/lvalues.rs ownership).
module tb;
    typedef struct packed {
        logic [7:0] hi;
        logic [7:0] lo;
    } pair_t;

    typedef struct {
        pair_t a;
        pair_t b;
        int    idx;
    } outer_t;

    pair_t p;
    outer_t u;

    initial begin
        p = '{hi: 8'hAA, lo: 8'h55};
        u.a = p;
        u.b = '{hi: 8'h11, lo: 8'h22};
        u.idx = 3;
        $display("u=%h%h %h%h %0d", u.a.hi, u.a.lo, u.b.hi, u.b.lo, u.idx);
        $finish(0);
    end
endmodule
