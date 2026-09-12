// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/disable_async.sv
// IEEE 1800-2009 16.15/16.16: disable iff is an unsampled asynchronous
// abort; a pending non-overlapped attempt is discarded before the next edge.
module tb;
    logic clk;
    logic antecedent;
    logic consequent;
    logic disabled;

    initial begin
        clk = 1'b0;
        antecedent = 1'b1;
        consequent = 1'b0;
        disabled = 1'b0;

        #1 clk = 1'b1;
        #1 disabled = 1'b1;
        #1 clk = 1'b0;
        #1 begin
            disabled = 1'b0;
            consequent = 1'b1;
        end
        #1 clk = 1'b1;
        #1 clk = 1'b0;
        #1 clk = 1'b1;
        #1 $finish(0);
    end

    resumed: assert property (
        @(posedge clk) disable iff (disabled) antecedent |=> consequent
    ) $display("DISABLE_PASS");
endmodule
