// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/sequence_matched.sv
// IEEE 1800-2009 §16.11: sequence `.matched` exposes the sequence endpoint;
// a two-cycle sequence matches on its second sampled clock tick, and the
// match is seen on the next tick of the destination clock (decision S39-D5),
// here the third tick.
module tb;
    logic clk;
    logic first;
    logic second;

    sequence pair;
        first ##1 second;
    endsequence

    matched: cover property (@(posedge clk) pair.matched)
        $display("MATCHED_PASS");

    initial begin
        clk = 1'b0;
        first = 1'b0;
        second = 1'b0;
        #1 first = 1'b1;
        #1 clk = 1'b1;
        #1 begin
            clk = 1'b0;
            first = 1'b0;
            second = 1'b1;
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
