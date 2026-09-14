// llg-test-fixture: tests/fixtures/sim/review_batch4/first_match_ties.sv
module tb;
    bit clk = 0, start = 1;
    int hits;
    sequence tied;
        int v;
        first_match((start, v = 1) or (start, v = 2)) ##1 (v == 2);
    endsequence
    cover property (@(posedge clk) tied) hits++;
    initial begin
        #1 clk = 1;
        #1 begin clk = 0; start = 0; end
        #1 clk = 1;
        #1;
        if (hits != 1) $fatal(1, "first_match discarded an earliest tied endpoint");
        $display("first_match tied endpoints ok");
        $finish(0);
    end
endmodule
