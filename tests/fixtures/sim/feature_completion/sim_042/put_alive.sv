// llg-test-fixture: SIM-042 pending delayed puts keep the simulation alive
// (decision S42-D5). put_alive.c ($put_alive) schedules the only future
// events, one from a value-change callback of an earlier put's write; the
// run has no $finish and ends after the last put. Expected
// output: put_alive.out, derived in readme.md.
`timescale 1ns/1ns
module tb;
    bit [3:0] d;
    always @(d) $display("%0t d=%0d", $time, d);
    initial $put_alive;
    final $display("final at %0t d=%0d", $time, d);
endmodule
