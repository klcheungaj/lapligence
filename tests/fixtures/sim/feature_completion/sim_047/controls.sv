// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/controls.sv
// SIM-047 step 3: $dumpfile, $dumpvars, $dumpon, $dumpoff, $dumpall and
// $dumpflush against end-of-step values (IEEE 1364-2001 18.1.1-18.1.6,
// IEEE 1800-2009 21.7.1.1-21.7.1.6; S47-D1, D2, D5, D6, D7).
`timescale 1ns/1ns
module tb;
    logic [3:0] v;
    logic s;
    real r;

    task show;
        $strobe("%0t v=%h s=%b r=%.1f", $time, v, s, r);
    endtask

    initial begin
        v = 4'h0;
        s = 1'b0;
        r = 0.0;
        $dumpfile("ignored.vcd");
        $dumpfile("trace.vcd");
        $dumpvars(0, tb);
        v = 4'h1;
        $dumpon;
        show;
        #1 v = 4'h2;
        v = 4'h3;
        s = 1'b1;
        $dumpfile("late.vcd");
        show;
        #1 v = 4'h4;
        $dumpoff;
        v = 4'h5;
        show;
        #1 $dumpoff;
        $dumpall;
        v = 4'h6;
        r = 2.5;
        show;
        #1 v = 4'h7;
        $dumpon;
        v = 4'h8;
        show;
        #1 $dumpall;
        s = 1'b0;
        $dumpflush;
        show;
        #1 $finish;
    end

    final begin
        v = 4'h9;
        $display("final v=%h", v);
    end
endmodule
