// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/sequence_unbounded.sv
// IEEE 1800-2009 16.7/16.8: goto repetition with an unbounded endpoint stays
// active until a sampled match instead of imposing a finite expansion limit.
module tb;
    logic clk;
    logic pulse;
    logic result;

    unbounded: assert property (@(posedge clk) pulse[->1:$] |-> result)
        $display("UNBOUNDED_PASS");

    initial begin
        clk = 1'b0;
        pulse = 1'b0;
        result = 1'b0;
        #1 clk = 1'b1;
        #1 begin
            clk = 1'b0;
            pulse = 1'b0;
        end
        #1 clk = 1'b1;
        #1 begin
            clk = 1'b0;
            pulse = 1'b1;
            result = 1'b1;
        end
        #1 clk = 1'b1;
        #1 $finish(0);
    end
endmodule
