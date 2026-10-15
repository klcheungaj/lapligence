// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/dump_extension.sv
// SIM-047 step 3: a $dumpfile name without the .fst extension is a VCD
// file, like the LRM's own `verilog.dump` example (S47-D7).
`timescale 1ns/1ns
module tb;
    logic v;
    initial begin
        v = 1'b0;
        $dumpfile("verilog.dump");
        $dumpvars;
        #1 v = 1'b1;
        $strobe("%0t v=%b", $time, v);
        #1 $finish;
    end
endmodule
