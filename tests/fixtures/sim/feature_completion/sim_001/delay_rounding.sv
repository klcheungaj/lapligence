// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_001/delay_rounding.sv
// IEEE 1800-2009 §§3.14.1, 9.4.1 and 20.4; IEEE 1364-2001 §9.7.1. tb has a
// 1ns unit and 100ps precision while `fine` makes the design tick 1ps, so
// local precision rounding (half away from zero) precedes conversion to
// scheduler ticks. Constant and runtime real delays, sub-precision delays
// (same slot), X/Z delays (zero) and runtime expressions that change between
// evaluations are each evaluated once per execution. Times print in ps.
module fine;
    timeunit 1ps;
    timeprecision 1ps;
endmodule

module tb;
    timeunit 1ns;
    timeprecision 100ps;
    fine f();
    real r;
    int k;
    logic [7:0] d;
    logic [3:0] xz;
    int evaluations = 0;

    function automatic int next_delay(int step);
        evaluations++;
        return step * 2;
    endfunction

    initial begin
        $timeformat(-12, 0, "", 0);
        #1.25 $display("const 1.25 %t", $realtime);
        #1.24 $display("const 1.24 %t", $realtime);
        #0.04 $display("const sub-precision %t", $realtime);
        #0.05 $display("const half precision %t", $realtime);
        #1.3ns $display("literal 1.3ns %t", $realtime);
        #250ps $display("literal 250ps %t", $realtime);
        #249ps $display("literal 249ps %t", $realtime);
        r = 1.25;
        #(r) $display("runtime 1.25 %t", $realtime);
        r = 0.04;
        #(r) $display("runtime sub-precision %t", $realtime);
        r = 2.75;
        #(r) $display("runtime 2.75 %t", $realtime);
        r = 0.3;
        #(r * 2) $display("runtime expr 0.6 %t", $realtime);
        xz = 4'bx1z0;
        #(xz) $display("x/z %t", $realtime);
        for (k = 1; k <= 3; k++) begin
            d = k * 3;
            #(d) $display("changed d=%0d %t", d, $realtime);
        end
        #(next_delay(2)) $display("call once evaluations=%0d %t", evaluations, $realtime);
        $finish(0);
    end
endmodule
