// Compare immediate unpacked elements; do not merge nested leaves separately.
module tb;
    typedef logic [6:0] matrix_t [2:1][-1:0];
    typedef struct packed { logic [3:0] tag; logic [3:0] data; } packed_element_t;
    typedef packed_element_t packed_array_t [0:1];
    typedef struct {
        int count = 7;
        logic [7:0] data = 8'ha5;
        logic [3:0] codes [2] = '{4'ha, 4'hb};
    } element_t;
    typedef element_t record_array_t [0:1];
    matrix_t a, b, result;
    packed_array_t pa, pb, pr;
    record_array_t ra, rb, rr;
    logic selector;

    function automatic matrix_t choose_matrix(input logic sel, input matrix_t x, y);
        return sel ? x : y;
    endfunction
    function automatic packed_array_t choose_packed(input logic sel, input packed_array_t x, y);
        return sel ? x : y;
    endfunction
    function automatic record_array_t choose_records(input logic sel, input record_array_t x, y);
        return sel ? x : y;
    endfunction

    initial begin
        a[2][-1] = 1; a[2][0] = 2; a[1][-1] = 4; a[1][0] = 5;
        b[2][-1] = 1; b[2][0] = 3; b[1][-1] = 4; b[1][0] = 5;
        selector = 1'bx;
        result = choose_matrix(selector, a, b);
        if (result[2][-1] !== 7'bx || result[2][0] !== 7'bx) $fatal(1, "whole differing row");
        if (result[1][-1] !== 7'd4 || result[1][0] !== 7'd5) $fatal(1, "equal row");
        pa[0] = 8'ha5; pb[0] = 8'ha6; pa[1] = 8'h5a; pb[1] = 8'h5a;
        pr = choose_packed(selector, pa, pb);
        if (pr[0] !== 8'hxx || pr[1] !== 8'h5a) $fatal(1, "whole packed-struct element");
        // Declaration defaults must still apply before the conditional.
        if (ra[0].count !== 7 || ra[0].data !== 8'ha5 || ra[0].codes[1] !== 4'hb)
            $fatal(1, "declaration defaults changed");
        ra[0].count = 9; rb[0].count = 9;
        ra[0].data = 8'ha5; rb[0].data = 8'ha6;
        ra[1].count = 4; rb[1].count = 4;
        ra[1].data = 8'h5a; rb[1].data = 8'h5a;
        rr = choose_records(selector, ra, rb);
        // The entire differing record defaults, ignoring explicit member
        // initializers: int is two-state zero; logic and its array are X.
        if (rr[0].count !== 0 || rr[0].data !== 8'hxx ||
            rr[0].codes[0] !== 4'hx || rr[0].codes[1] !== 4'hx)
            $fatal(1, "default-uninitialized record");
        if (rr[1].count !== 4 || rr[1].data !== 8'h5a ||
            rr[1].codes[0] !== 4'ha || rr[1].codes[1] !== 4'hb)
            $fatal(1, "equal record preserved");
        $display("array conditional nested defaults passed");
        $finish(0);
    end
endmodule
