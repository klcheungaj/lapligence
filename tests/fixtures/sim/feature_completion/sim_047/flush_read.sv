// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/flush_read.sv
// SIM-047 step 3/4: after $dumpflush another program reads the dump during
// the simulation and sees every value up to that point (IEEE 1364-2001
// 18.1.6). FILE selects the dump file; the copy is made with $system.
`timescale 1ns/1ns
module tb;
    logic [3:0] a;
    int status;
    initial begin
        a = 4'h0;
        $dumpfile(`FILE);
        $dumpvars(0, tb);
        #1 a = 4'h1;
        #1 a = 4'h2;
        $dumpflush;
        status = $system(`COPY);
        $display("copy status=%0d", status);
        #1 a = 4'h3;
        #1 $finish;
    end
endmodule
