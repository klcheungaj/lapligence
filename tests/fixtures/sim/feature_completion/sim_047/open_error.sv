// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/open_error.sv
// SIM-047 step 3: a dump file that cannot be created is reported once; the
// simulation still runs to its end and the run fails (S47-D11).
`timescale 1ns/1ns
module tb;
    logic v;
    initial begin
        v = 1'b0;
        $dumpfile("missing_directory/trace.vcd");
        $dumpvars;
        #1 v = 1'b1;
        $dumpflush;
        $display("%0t v=%b", $time, v);
        #1 v = 1'b0;
        $display("%0t v=%b", $time, v);
        $finish;
    end
endmodule
