// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/h25_multiclock.sv
// IEEE 1800-2009 16.14.1: a legal zero/one-delay sequence boundary may
// inherit a leading clock and switch to a second explicit sampled clock.
module tb;
    logic clk_a;
    logic clk_b;
    logic first;
    logic second;

    sequence first_segment;
        @(posedge clk_a) first;
    endsequence

    property cross_clock;
        first_segment ##1 @(posedge clk_b) second;
    endproperty

    property cross_clock_zero;
        @(posedge clk_a) first ##0 @(posedge clk_b) second;
    endproperty

    cross_assert: assert property (cross_clock) $display("H25_MULTICLOCK_PASS");
    cross_zero_assert: assert property (cross_clock_zero)
        $display("H25_MULTICLOCK_ZERO_PASS");

    initial begin
        clk_a = 1'b0;
        clk_b = 1'b0;
        first = 1'b0;
        second = 1'b0;
        #1 begin
            first = 1'b1;
            second = 1'b1;
        end
        #1 begin
            clk_a = 1'b1;
            clk_b = 1'b1;
        end
        #1 clk_a = 1'b0;
        #1 clk_b = 1'b0;
        #1 $finish(0);
    end
endmodule
