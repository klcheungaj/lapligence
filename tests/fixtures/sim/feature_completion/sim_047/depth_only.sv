// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/depth_only.sv
// SIM-047 step 2: $dumpvars(levels) without a scope list, called from a
// child instance, applies the levels below the top-level instance, like
// $dumpvars without arguments dumps the complete model (S47-D4).
`timescale 1ns/1ns
module child;
    logic [1:0] c;
    initial begin
        c = 2'd1;
        $dumpfile("trace.vcd");
        $dumpvars(1);
        #1 c = 2'd2;
    end
endmodule

module tb;
    logic [1:0] t;
    child u();
    initial begin
        t = 2'd0;
        #1 t = 2'd3;
        $strobe("%0t t=%b c=%b", $time, t, u.c);
        #1 $finish;
    end
endmodule
