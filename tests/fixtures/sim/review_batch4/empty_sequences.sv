// llg-test-fixture: tests/fixtures/sim/review_batch4/empty_sequences.sv
module tb;
    bit clk = 0, start = 1, b = 0, c = 1;
    int left_hits, right_hits, both_hits, forbidden_hits;
    cover property (@(posedge clk) start ##0 (b[*0] ##1 c)) left_hits++;
    cover property (@(posedge clk) start ##0 (c ##1 b[*0])) right_hits++;
    cover property (@(posedge clk) start ##0 (b[*0] ##2 b[*0])) both_hits++;
    cover property (@(posedge clk) start ##0 b[*0:1] ##0 c) forbidden_hits++;
    initial begin
        #1 clk = 1;
        #1 begin clk = 0; start = 0; c = 0; end
        #1 clk = 1;
        #1;
        if (left_hits != 1 || right_hits != 1 || both_hits != 1 || forbidden_hits != 0)
            $fatal(1, "empty sequence boundary left=%0d right=%0d both=%0d zero=%0d", left_hits, right_hits, both_hits, forbidden_hits);
        $display("empty sequence boundaries ok");
        $finish(0);
    end
endmodule
