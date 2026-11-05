// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/named_statics.sv
// SIM-047 A03: $dumpvars names one static block or subroutine variable or one
// event (SV 21.7.1.2: "individual variables"); the rest of the design stays
// out of the dump. Depth does not apply to a variable.
module tb;
    int skipped;
    event pulse;
    function int sum(int step);
        static int total;
        total = total + step;
        return total;
    endfunction
    initial begin : main
        static int count = 1;
        $dumpvars(1, tb.main.count, tb.sum.total, tb.pulse);
        #1 count = sum(2);
        ->pulse;
        skipped = count;
        #1 $display("count=%0d skipped=%0d", count, skipped);
        $finish;
    end
endmodule
