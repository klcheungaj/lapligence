// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/unsupported_repetition.sv
// IEEE 1800-2009 16.7/16.8: consecutive repetition consumes two adjacent
// sampled clock ticks and preserves the endpoint for an overlapped implication.
module tb;
    logic clk;
    logic signal_a;
    logic signal_b;

    repeated: assert property (@(posedge clk) signal_a[*2] |-> signal_b)
        $display("REPETITION_PASS");

    initial begin
        clk = 1'b0;
        signal_a = 1'b0;
        signal_b = 1'b0;
        #1 begin
            signal_a = 1'b1;
            signal_b = 1'b1;
        end
        #1 clk = 1'b1;
        #1 clk = 1'b0;
        #1 clk = 1'b1;
        #1 clk = 1'b0;
        #1 $finish(0);
    end
endmodule
