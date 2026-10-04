// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_001/observed_reentry.sv
// IEEE 1800-2009 §§4.5, 16.5 and 24.3.1: a concurrent assertion clocked by a
// signal that only program (Reactive) code changes is still evaluated in the
// Observed region of the same time slot: the scheduler iterates from the
// reactive region set back around the outer loop through Observed. The
// action block runs in Reactive and reads current values; the property reads
// Preponed samples.
program p;
    initial begin
        #10 tb.n = 1;
        tb.clk = 1;
        #10 tb.clk = 0;
        #10 tb.clk = 1;
        #10 tb.n = 0;
        tb.clk = 0;
        // Stay live so tb's $finish, not program completion, ends the run.
        #20;
    end
endprogram

module tb;
    logic clk = 0;
    int n = 0;
    p p0();

    // Sampled n is 0 at t=10 (fails) and 1 at t=30 (passes).
    a1: assert property (@(posedge clk) n > 0)
        $display("pass t=%0d n=%0d", $time, n);
    else
        $display("fail t=%0d n=%0d", $time, n);

    initial begin
        #50;
        $display("done");
        $finish(0);
    end
endmodule
