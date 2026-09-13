// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/unsupported_sequence_status.sv
// IEEE 1800-2009 16.9.3/16.13: sequence match endpoints are not guessed as
// sampled Boolean values while the general sequence engine remains deferred.
module tb;
    logic clk;
    logic value;
    sequence s;
        value;
    endsequence

    bad: assert property (@(posedge clk) s.matched);

    initial begin
        clk = 1'b0;
        value = 1'b0;
        #1 clk = 1'b1;
        #1 $finish(0);
    end
endmodule
