// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_001/negative_real_constant.sv
// IEEE 1364-2001 §9.7.1 and IEEE 1800-2009 §9.4.1: a constant negative real
// delay converts like a runtime one. -2.6 rounds to -3 local ticks (half away
// from zero), which is 2^64-3 as unsigned time; -0.4 rounds to zero.
module tb;
    timeunit 1s;
    timeprecision 1s;
    initial begin
        #(-0.4) $display("rounded to zero %0d", $time);
        #(-2.6) $display("constant %0d", $time);
        #1 $display("after %0d", $time);
        $finish(0);
    end
endmodule
