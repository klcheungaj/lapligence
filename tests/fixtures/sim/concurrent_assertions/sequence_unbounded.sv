// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/sequence_unbounded.sv
// IEEE 1800-2009 16.7/16.8: goto repetition with an unbounded endpoint stays
// active until a sampled match instead of imposing a finite expansion limit.
// `cover sequence` reports each match (16.15.3): the attempts of ticks 1-3
// all match on the pulse of tick 3. As an implication antecedent the same
// sequence could match again later, so an assert attempt would stay pending
// (16.13.6: every match must be followed by a passing consequent).
module tb;
    logic clk;
    logic pulse;

    unbounded: cover sequence (@(posedge clk) pulse[->1:$])
        $display("UNBOUNDED_PASS");

    initial begin
        clk = 1'b0;
        pulse = 1'b0;
        #1 clk = 1'b1;
        #1 begin
            clk = 1'b0;
            pulse = 1'b0;
        end
        #1 clk = 1'b1;
        #1 begin
            clk = 1'b0;
            pulse = 1'b1;
        end
        #1 clk = 1'b1;
        #1 $finish(0);
    end
endmodule
