// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_001/negative_real_overflow.sv
// tb's -1.0 is 2^64-1 local ticks of 1ns (SV 9.4.1, a negative delay is a
// two's-complement unsigned time), which cannot be represented in the 1ps
// design tick: the delay has no 64-bit tick count, so the process waits one
// tick, with a warning, and continues (decision S01-D1).
module fine;
    timeunit 1ps;
    timeprecision 1ps;
endmodule

module tb;
    timeunit 1ns;
    timeprecision 1ns;
    fine f();
    real d = -1.0;
    initial begin
        $display("before");
        #(d);
        $display("after");
    end
endmodule
