// IEEE 1800-2009 6.19.5: first/last/next/prev/num/name use declaration order,
// next and prev wrap modulo the member count, and a value with no member
// returns the enumeration default (X for a four-state base, zero for a
// two-state base) with an empty name. The expected trace is an independent
// declaration-order oracle in tests/sim_g1_closure.rs.
module tb;
    typedef enum logic signed [7:0] {
        E_MIN  = -8'sd128,
        E_NEG  = -8'sd1,
        E_ZERO = 8'sd0,
        E_POS  = 8'sd50,
        E_MAX  = 8'sd127
    } sparse_t;

    typedef enum bit [2:0] {
        T_LO  = 3'd1,
        T_MID = 3'd4,
        T_HI  = 3'd7
    } two_t;

    sparse_t s;
    two_t t;

    initial begin
        $display("first=%b last=%b num=%0d", s.first(), s.last(), s.num());

        s = E_MIN;
        $display("min name=%s next=%b prev=%b", s.name(), s.next(), s.prev());
        s = E_MAX;
        $display("max name=%s next=%b prev=%b", s.name(), s.next(), s.prev());
        s = E_ZERO;
        $display("zero name=%s next0=%b next1=%b next4=%b next5=%b next6=%b",
                 s.name(), s.next(0), s.next(1), s.next(4), s.next(5), s.next(6));
        $display("zero prev0=%b prev1=%b prev4=%b prev6=%b",
                 s.prev(0), s.prev(1), s.prev(4), s.prev(6));
        $display("zero nextx=%b prevx=%b", s.next(32'hx), s.prev(32'hz));

        s = sparse_t'(8'sd7);
        $display("invalid name=[%s] next=%b prev=%b", s.name(), s.next(), s.prev());
        s = sparse_t'(8'hzz);
        $display("highz name=[%s] next=%b prev=%b", s.name(), s.next(), s.prev());
        s = sparse_t'(8'hx5);
        $display("partial name=[%s] next=%b prev=%b", s.name(), s.next(), s.prev());

        t = T_MID;
        $display("two mid name=%s next=%b prev=%b num=%0d",
                 t.name(), t.next(), t.prev(), t.num());
        t = two_t'(3'd2);
        $display("two invalid name=[%s] next=%b prev=%b", t.name(), t.next(), t.prev());
        $display("two invalid first=%b last=%b", t.first(), t.last());

        $display("PASS enum_navigation_sparse");
        $finish(0);
    end
endmodule
