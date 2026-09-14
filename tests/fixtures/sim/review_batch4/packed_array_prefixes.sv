// llg-test-fixture: tests/fixtures/sim/review_batch4/packed_array_prefixes.sv
module tb;
    logic a = 0, b = 0;
    logic [3:0] words [1:2];
    logic [3:0] mirror;
    always_comb words[1][1:0] = {a,b};
    always_comb words[1][3:2] = {b,a};
    always_comb words[2] = 4'b1001;
    always_comb mirror = words[1];
    initial begin
        #1;
        if (mirror !== 0) $fatal(1, "array prefix initial");
        a = 1;
        #1;
        if (mirror !== 4'b0110 || words[2] !== 4'b1001) $fatal(1, "array selected dependency notification");
        b = 1;
        #1;
        if (mirror !== 4'b1111) $fatal(1, "array upper slice update");
        $display("packed array prefixes ok");
        $finish(0);
    end
endmodule
