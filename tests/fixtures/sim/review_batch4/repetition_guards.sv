// llg-test-fixture: tests/fixtures/sim/review_batch4/repetition_guards.sv
module tb;
    bit clk = 0, start = 1, b = 1, c = 0;
    int extra_hits, range_hits;
    cover property (@(posedge clk) start ##0 b[=1] ##1 c) extra_hits++;
    cover property (@(posedge clk) start ##0 b[->1:2] ##1 c) range_hits++;
    initial begin
        #1 clk = 1;
        #1 begin clk = 0; start = 0; end
        #1 clk = 1;
        #1 begin clk = 0; b = 0; c = 1; end
        #1 clk = 1;
        #1;
        if (extra_hits != 0 || range_hits != 1) $fatal(1, "repetition skipped an occurrence or a legal endpoint");
        $display("guarded repetition endpoints ok");
        $finish(0);
    end
endmodule
