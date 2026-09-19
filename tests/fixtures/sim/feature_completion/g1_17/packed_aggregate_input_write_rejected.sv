// IEEE 1800-2009 13.4.2/13.5.2: a by-value input formal is a local copy.
typedef struct packed {
    logic [7:0] hi;
    logic [7:0] lo;
} pair_t;

module tb;
    pair_t v;

    function automatic logic [7:0] f(input pair_t p);
        p.hi = 8'h01;
        if (p.hi !== 8'h01) $fatal(1, "packed input copy is not writable");
        f = p.lo;
    endfunction

    initial begin
        v.hi = 8'h10;
        v.lo = 8'h20;
        if (f(v) !== 8'h20 || v.hi !== 8'h10 || v.lo !== 8'h20)
            $fatal(1, "packed input write changed caller");
        $display("packed input copy passed");
        $finish(0);
    end
endmodule
