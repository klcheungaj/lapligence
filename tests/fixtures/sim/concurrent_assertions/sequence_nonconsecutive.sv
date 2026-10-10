// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/sequence_nonconsecutive.sv
// IEEE 1800-2009 16.7/16.8: nonconsecutive repetition permits sampled gaps
// between required matches while retaining the final endpoint.
// `pulse[=2]` keeps matching while `pulse` stays low after the second pulse,
// so the tick-1 attempt passes once on tick 3, when a third pulse ends its
// antecedent (16.13.6); later attempts are still pending at $finish.
module tb;
    logic clk;
    logic pulse;
    logic result;

    nonconsecutive: assert property (@(posedge clk) pulse[=2] |-> result)
        $display("NONCONSECUTIVE_PASS");

    initial begin
        clk = 1'b0;
        pulse = 1'b0;
        result = 1'b0;
        #1 pulse = 1'b1;
        #1 clk = 1'b1;
        #1 begin
            clk = 1'b0;
            pulse = 1'b0;
        end
        #1 begin
            pulse = 1'b1;
            result = 1'b1;
        end
        #1 clk = 1'b1;
        #1 clk = 1'b0;
        #1 clk = 1'b1;
        #1 clk = 1'b0;
        #1 $finish(0);
    end
endmodule
