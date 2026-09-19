module tb;
    typedef struct packed { logic [7:0] hi, lo; } pair_t;
    typedef struct { pair_t pair; int count; } value_t;
    value_t source;
    logic [7:0] observed;
    int combined;
    function automatic int total(input value_t value);
        return int'(value.pair.lo) + value.count;
    endfunction
    always_comb observed = source.pair.lo;
    always_comb combined = total(source);
    initial begin
        source = '{pair:16'h1234, count:2};
        #1;
        $display("observed=%h combined=%0d", observed, combined);
        source.pair.lo = 8'h56;
        #1;
        $display("observed=%h combined=%0d", observed, combined);
        source.count = 9;
        #1;
        $display("observed=%h combined=%0d", observed, combined);
        $finish(0);
    end
endmodule
