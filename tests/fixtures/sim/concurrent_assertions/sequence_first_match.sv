// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/sequence_first_match.sv
// IEEE 1800-2009 16.8/16.9: first_match selects the earliest inner endpoint
// while retaining the following concatenation endpoint.
module tb;
    logic clk;
    logic first;
    logic second;
    logic result;

    earliest: assert property (
        @(posedge clk) (first_match(first[*1:2]) ##1 second) |-> result
    ) $display("FIRST_MATCH_PASS");

    initial begin
        clk = 1'b0;
        first = 1'b0;
        second = 1'b0;
        result = 1'b0;
        #1 first = 1'b1;
        #1 clk = 1'b1;
        #1 begin
            clk = 1'b0;
            first = 1'b0;
            second = 1'b1;
            result = 1'b1;
        end
        #1 clk = 1'b1;
        #1 begin
            clk = 1'b0;
            second = 1'b0;
        end
        #1 clk = 1'b1;
        #1 $finish(0);
    end
endmodule
