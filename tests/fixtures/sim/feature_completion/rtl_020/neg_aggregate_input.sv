// IEEE 1800-2009 29.3.1, 29.8: a packed structure is a vector value, not a
// scalar terminal; one of its one-bit members would be legal.
primitive p(y, a, b);
    output y;
    input a, b;
    table
        0 ? : 0;
        1 ? : 1;
    endtable
endprimitive

module tb;
    typedef struct packed {
        logic f;
        logic g;
    } pair_t;
    pair_t pair;
    logic b;
    wire y;
    p u(y, pair, b);
    initial begin
        pair = '0; b = 0;
        #1 $display("%b", y);
        $finish(0);
    end
endmodule
