// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/no_dumpvars.sv
// SIM-047 step 3: without $dumpvars no dump starts, so no file is written,
// whatever other dump tasks run (S47-D5, S47-D7).
`timescale 1ns/1ns
module tb;
    logic [3:0] v;
    initial begin
        v = 4'h1;
        $dumpfile("trace.vcd");
        $dumplimit(100);
        $dumpon;
        $dumpall;
        #1 v = 4'h2;
        $dumpflush;
        $dumpoff;
        $display("%0t v=%h", $time, v);
        #1 $finish;
    end
endmodule
