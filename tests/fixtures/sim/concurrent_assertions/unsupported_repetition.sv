// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/unsupported_repetition.sv
// IEEE 1800-2009 16.6: repetition is retained in the owned assertion graph,
// then rejected by the bounded single-cycle lowerer rather than treated as a
// one-cycle immediate assertion.
module tb;
    logic clk;
    logic signal_a;

    bad: assert property (@(posedge clk) signal_a[*2]);

    initial begin
        clk = 1'b0;
        signal_a = 1'b0;
        $finish(0);
    end
endmodule
