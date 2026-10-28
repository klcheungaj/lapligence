// llg-test-fixture: SIM-042 delayed vpi_put_value scheduling (decision
// S42-D5). put_region.c ($put_region) puts with an inertial, a transport and
// a pure transport delay; each pending put is an Active-region event of its
// target slot, ordered with process wakeups by when it was scheduled. It
// also cancels one event in each mode, lets an inertial and a transport put
// remove earlier events, and leaves one put pending at $finish. Expected
// output: put_region.out, derived in readme.md.
`timescale 1ns/1ns
module tb;
    bit [3:0] d, e, f;
    bit [3:0] ci, ct, cp;
    bit [3:0] late;
    initial #2 $display("2 early d=%0d e=%0d f=%0d", d, e, f);
    always @(d) $display("%0t d=%0d", $time, d);
    always @(ci) $display("%0t ci=%0d", $time, ci);
    always @(ct) $display("%0t ct=%0d", $time, ct);
    always @(cp) $display("%0t cp=%0d", $time, cp);
    always @(late) $display("%0t late=%0d", $time, late);
    initial begin
        $put_region;
        #2 $display("2 late d=%0d e=%0d f=%0d", d, e, f);
        #0 $display("2 inactive d=%0d e=%0d f=%0d", d, e, f);
        $strobe("2 postponed d=%0d e=%0d f=%0d", d, e, f);
        #7 $finish;
    end
    initial begin
        #2;
        #0;
        #0 $display("2 late inactive d=%0d", d);
    end
    initial #1 d <= 4'd9;
    final $display("final at %0t late=%0d", $time, late);
endmodule
