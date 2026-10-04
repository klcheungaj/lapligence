// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_001/negative_real_delay.sv
// IEEE 1364-2001 §9.7.1 and IEEE 1800-2009 §9.4.1: a negative delay is not
// illegal; its value, rounded to the local precision, is reinterpreted as a
// 64-bit two's-complement unsigned time. With a 1s unit and precision the
// design tick is one second, so -1.0 waits 2^64-1 ticks and a constant -3.0
// waits 2^64-3 ticks.
module tb;
    timeunit 1s;
    timeprecision 1s;
    real d = -1.0;
    initial begin
        if (d < 0.0) begin
            #(d);
            $display("runtime %0d", $time);
            $finish(0);
        end
    end
endmodule
