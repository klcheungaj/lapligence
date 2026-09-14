// llg-test-fixture: tests/fixtures/sim/review_batch4/first_match_scope.sv
module tb;
    bit clk = 0, start = 1, a = 1, b = 0, c = 0, d = 1, e = 0;
    int hits;
    cover property (@(posedge clk) start ##0
        ((first_match(a ##[1:2] b) ##1 c) or (d ##3 e))) hits++;
    initial begin
        #1 clk = 1;
        #1 begin clk = 0; start = 0; a = 0; d = 0; b = 1; end
        #1 clk = 1;
        #1 begin clk = 0; b = 0; end
        #1 clk = 1;
        #1 begin clk = 0; e = 1; end
        #1 clk = 1;
        #1;
        if (hits != 1) $fatal(1, "nested first_match pruned sibling alternative");
        $display("first_match scope ok");
        $finish(0);
    end
endmodule
