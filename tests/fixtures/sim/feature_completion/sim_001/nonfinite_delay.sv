// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_001/nonfinite_delay.sv
// A nonfinite real delay has no 64-bit tick count: the process waits one
// tick, with a warning, and continues (decision S01-D1).
module tb;
    real d = 1.0e308;
    initial begin
        $display("before");
        d = d * 10.0;
        #(d);
        $display("after %0t", $realtime);
    end
endmodule
