// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_001/time_limit_nba.sv
// Owner resource policy: a delayed nonblocking update past the 64-bit tick
// range fails at issue time, before it enters the timed NBA queue.
module tb;
    timeunit 1fs;
    timeprecision 1fs;
    logic [7:0] q = 0;
    initial begin
        #(64'hFFFF_FFFF_FFFF_FFF0);
        $display("near %0d", $time);
        q <= #15 8'd1;
        q <= #16 8'd2;
        $display("unreachable");
    end
endmodule
