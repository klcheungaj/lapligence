// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_001/neg_negative_real_overflow.sv
// Resource boundary, not a language error: tb's -1.0 is 2^64-1 local ticks of
// 1ns, which cannot be represented in the 1ps design tick. The runtime
// reports the 64-bit tick limit instead of rejecting the negative value.
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
        $display("unreachable");
    end
endmodule
