// llg-test-fixture: tests/fixtures/sim/process_semantics/member_prefix.sv
// IEEE 1800-2009 Sections 7.2, 9.2.2.2 and 13.4: bounded unpacked members
// keep independent storage identity for always_comb reads and writers.
module tb;
    typedef struct {
        logic [3:0] hi;
        logic [3:0] lo;
    } pair_t;

    pair_t pair;
    logic [3:0] hi_input;
    logic [3:0] lo_input;
    logic [3:0] observed;

    function automatic logic [3:0] read_hi();
        read_hi = pair.hi;
    endfunction

    always_comb pair.hi = hi_input;
    always_comb pair.lo = lo_input;
    always_comb observed = read_hi();

    initial begin
        hi_input = 4'ha;
        lo_input = 4'h3;
        #1 $display("initial observed=%h hi=%h lo=%h", observed, pair.hi, pair.lo);
        lo_input = 4'h5;
        #1 $display("lo observed=%h hi=%h lo=%h", observed, pair.hi, pair.lo);
        hi_input = 4'hc;
        #1 $display("hi observed=%h hi=%h lo=%h", observed, pair.hi, pair.lo);
        $finish(0);
    end
endmodule
