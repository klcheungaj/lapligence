// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/sequence_ranges.sv
// IEEE 1800-2009 16.7/16.8: zero-delay concatenation shares an endpoint,
// while an inclusive ## delay range preserves both its lower and upper bound.
// An implication attempt passes once, after its antecedent can match no more
// (16.13.6): the tick-1 attempt of `ranged` matches at tick 2 and resolves on
// tick 3, where the `##2` alternative fails. Ticks 2 and 3 pass vacuously.
module tb;
    logic clk;
    logic first;
    logic second;
    logic result;

    zero: assert property (@(posedge clk) (first ##0 second) |-> result)
        $display("ZERO_PASS");
    ranged: assert property (@(posedge clk) (first ##[1:2] second) |-> result)
        $display("RANGED_PASS");

    initial begin
        clk = 1'b0;
        first = 1'b0;
        second = 1'b0;
        result = 1'b0;
        #1 begin
            first = 1'b1;
            second = 1'b1;
            result = 1'b1;
        end
        #1 clk = 1'b1;
        #1 begin
            clk = 1'b0;
            first = 1'b0;
            second = 1'b0;
        end
        #1 second = 1'b1;
        #1 clk = 1'b1;
        #1 begin
            clk = 1'b0;
            second = 1'b0;
        end
        #1 clk = 1'b1;
        #1 clk = 1'b0;
        #1 $finish(0);
    end
endmodule
