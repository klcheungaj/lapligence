// IEEE 1800-2009 6.22/7.3: a packed union's members overlay one activation
// value passed through a formal, including a nested packed struct member.
typedef struct packed {
    logic [7:0] hi;
    logic [7:0] lo;
} pair_t;

typedef union packed {
    logic [15:0] raw;
    pair_t pair;
} word_t;

module tb;
    word_t v;

    function automatic logic [15:0] swap(input word_t x);
        swap = {x.pair.lo, x.pair.hi};
    endfunction

    initial begin
        v.raw = 16'h1234;
        $display("swap=%04h raw=%04h lo=%02h", swap(v), v.raw, v.pair.lo);
        $finish(0);
    end
endmodule
