// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/checkpoint_timing.sv
// $dumpon, $dumpoff and $dumpall checkpoints are written at the end of their
// time step, after the step's records and with its final values (decision
// S47-D5, user decision 2026-11-19). The dump state at the start and at the
// end of the step decides the checkpoint; an off-on or on-off pair within one
// step writes none, and several $dumpall calls one.
`timescale 1ns/1ns
`ifndef FILE
`define FILE "timing.vcd"
`endif
module tb;
    logic s;
    logic [3:0] v;
    real r;
    event e;

    initial begin
        s = 1'b0;
        v = 4'h1;
        r = 0.5;
        $dumpfile(`FILE);
        $dumpvars(0, tb);
        $dumpall;
        v = 4'h2;
        #1 v = 4'h3;
        $dumpoff;
        v = 4'h4;
        #1 v = 4'h5;
        #1 $dumpon;
        v = 4'h6;
        #0 v = 4'h7;
        #1 $dumpoff;
        #0 v = 4'h8;
        s = 1'b1;
        #1 $dumpon;
        $dumpoff;
        v = 4'h9;
        #1 $dumpon;
        $dumpall;
        v = 4'ha;
        #1 $dumpoff;
        $dumpon;
        v = 4'hb;
        #1 $dumpall;
        $dumpall;
        r = 1.5;
        -> e;
        #1 v = 4'hc;
        $dumpall;
        v = 4'hb;
        #1 $display("done v=%h s=%b r=%.1f", v, s, r);
        $finish;
    end
endmodule
