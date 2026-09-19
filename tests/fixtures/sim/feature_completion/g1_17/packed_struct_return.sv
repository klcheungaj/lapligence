// IEEE 1800-2009 6.22, 13.5.1: a fixed packed struct is a legal function
// return type. The return slot is a single activation-owned value, and the
// caller copies the whole result into its destination.
typedef struct packed {
    logic [7:0] hi;
    logic [7:0] lo;
} pair_t;

module tb;
    pair_t src;
    pair_t swapped;
    pair_t extended;

    function automatic pair_t swap(input pair_t p);
        swap = {p.lo, p.hi};
    endfunction

    function automatic pair_t widen(input pair_t p);
        widen = {p.hi + 8'd1, p.lo ^ 8'hff};
    endfunction

    initial begin
        src.hi = 8'h12;
        src.lo = 8'h34;
        swapped = swap(src);
        extended = widen(src);
        $display("swap=%02h%02h widen=%02h%02h src=%02h%02h",
                 swapped.hi, swapped.lo, extended.hi, extended.lo, src.hi, src.lo);
        $finish(0);
    end
endmodule
