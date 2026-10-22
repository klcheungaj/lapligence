// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_001/time_limit_nba.sv
// A delayed nonblocking update whose time would pass the last 64-bit tick
// takes effect in the NBA region of the current time slot; one that reaches
// exactly 2^64-1 is scheduled there (decision S01-D2).
module tb;
    timeunit 1fs;
    timeprecision 1fs;
    logic [7:0] q;
    always @(q) $display("q=%0d at %0d", q, $time);
    initial begin
        #(64'hFFFF_FFFF_FFFF_FFF0);
        $display("near %0d", $time);
        q <= #15 8'd1;
        q <= #16 8'd2;
        $display("issued");
    end
endmodule
