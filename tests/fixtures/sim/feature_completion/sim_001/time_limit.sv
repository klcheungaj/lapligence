// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_001/time_limit.sv
// Owner resource policy for IEEE 1800-2009 §§4.3 and 9.4.1: simulation time
// is a 64-bit tick count. A delay that reaches exactly 2^64-1 schedules; one
// that would pass it fails before anything is queued.
module tb;
    timeunit 1fs;
    timeprecision 1fs;
    initial begin
        #(64'hFFFF_FFFF_FFFF_FFF0);
        $display("near %0d", $time);
        #15 $display("max %0d", $time);
        #1 $display("unreachable");
    end
endmodule
