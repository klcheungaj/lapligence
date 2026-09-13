// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/sequence_combinators.sv
// IEEE 1800-2009 16.8/16.9: Boolean sequence composition preserves the
// sampled endpoint for or, and, intersect, throughout, and within.
module tb;
    logic clk;
    logic left;
    logic right;
    logic result;

    seq_or: assert property (@(posedge clk) (left or right) |-> result)
        $display("OR_PASS");
    seq_and: assert property (@(posedge clk) (left and right) |-> result)
        $display("AND_PASS");
    seq_intersect: assert property (@(posedge clk) (left intersect right) |-> result)
        $display("INTERSECT_PASS");
    seq_throughout: assert property (@(posedge clk) (left throughout right) |-> result)
        $display("THROUGHOUT_PASS");
    seq_within: assert property (@(posedge clk) (left within right) |-> result)
        $display("WITHIN_PASS");

    initial begin
        clk = 1'b0;
        left = 1'b0;
        right = 1'b0;
        result = 1'b0;
        #1 begin
            left = 1'b1;
            right = 1'b1;
            result = 1'b1;
        end
        #1 clk = 1'b1;
        #1 clk = 1'b0;
        #1 $finish(0);
    end
endmodule
