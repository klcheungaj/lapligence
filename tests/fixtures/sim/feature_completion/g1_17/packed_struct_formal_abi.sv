// IEEE 1800-2009 6.22, 13.4.2, 13.5.1: a fixed packed structure crosses a
// function formal by value. Member selects address the activation's own copy,
// output/inout formals copy back once at return, and a by-value input formal
// is a read-only view of the caller's independently owned value.
typedef struct packed {
    logic [7:0] hi;
    logic [7:0] lo;
} pair_t;

module tb;
    pair_t src;
    pair_t acc;
    pair_t out;
    pair_t filled;
    logic [7:0] mixed;

    function automatic logic [7:0] mix(input pair_t p, inout pair_t a, output pair_t o);
        a.lo = a.lo + p.lo;
        o.hi = a.hi;
        o.lo = a.lo;
        mix = p.hi ^ p.lo;
    endfunction

    function automatic void fill(output pair_t o, input logic [7:0] v);
        o = 16'h0000;   // whole-formal write
        o.hi = v;       // then a member write observes the same storage
    endfunction

    function automatic logic [15:0] combine(input pair_t p);
        combine = {p.hi, p.lo};
    endfunction

    initial begin
        logic [15:0] c1;
        logic [15:0] c2;
        src.hi = 8'h10;
        src.lo = 8'h05;
        acc.hi = 8'h20;
        acc.lo = 8'h02;
        mixed = mix(src, acc, out);
        $display("mix=%02h src=%02h%02h acc=%02h%02h out=%02h%02h",
                 mixed, src.hi, src.lo, acc.hi, acc.lo, out.hi, out.lo);
        fill(filled, 8'h7a);
        $display("fill=%02h%02h", filled.hi, filled.lo);
        c1 = combine(src);
        c2 = combine(src);
        $display("combine=%04h,%04h src=%02h%02h", c1, c2, src.hi, src.lo);
        $finish(0);
    end
endmodule
