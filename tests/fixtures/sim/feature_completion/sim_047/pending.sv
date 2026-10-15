// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/pending.sv
// SIM-047 step 3: controls before the first $dumpvars have no effect, and a
// $dumpoff in the step of the first $dumpvars starts the dump off (S47-D5).
`timescale 1ns/1ns
module tb;
    logic [3:0] v;
    initial begin
        v = 4'h1;
        $dumpfile("trace.vcd");
        $dumpon;
        $dumpall;
        $dumpflush;
        $dumpoff;
        #1 v = 4'h2;
        $dumpvars(0, tb);
        $dumpoff;
        v = 4'h3;
        $strobe("%0t v=%h", $time, v);
        #1 v = 4'h4;
        $dumpon;
        $strobe("%0t v=%h", $time, v);
        #1 $finish;
    end
endmodule
