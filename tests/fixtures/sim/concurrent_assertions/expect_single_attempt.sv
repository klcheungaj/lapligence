// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/expect_single_attempt.sv
// IEEE 1800-2009 16.18: "the expect statement starts a single thread of
// evaluation for the given property on the subsequent clocking event". An
// initial block holding only an expect still executes it, and later clock
// ticks start no further attempts: the predicate-path `go |=> ok` attempt of
// tick 1 fails at tick 2 even though `go` is 0 there (a tick-2 attempt would
// pass vacuously first), and the sequence-path attempt fails at tick 3.
module tb;
    logic clk = 1'b0;
    logic go = 1'b0;
    logic ok = 1'b0;
    int t = 0;

    initial
        expect (@(posedge clk) go |=> ok)
            $display("PREDICATE_PASS %0d", t);
        else
            $display("PREDICATE_FAIL %0d", t);

    initial begin
        expect (@(posedge clk) go ##[1:2] ok |-> !ok)
            $display("SEQUENCE_PASS %0d", t);
        else
            $display("SEQUENCE_FAIL %0d", t);
    end

    initial begin
        for (int k = 1; k <= 4; k++) begin
            t = k;
            go = k == 1;
            ok = k == 3;
            #5 clk = 1'b1;
            #5 clk = 1'b0;
        end
        $finish(0);
    end
endmodule
