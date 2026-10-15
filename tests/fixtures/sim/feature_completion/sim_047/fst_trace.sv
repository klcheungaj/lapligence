// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/fst_trace.sv
// SIM-047 step 4: the FST counterpart of the control sequence: values,
// $dumpoff/$dumpon activity and $dumplimit through GTKWave's reader. LIMIT
// selects a dump size limit in bytes.
`timescale 1ns/1ns
module tb;
    logic [7:0] v;
    logic s;
    real r;
    initial begin
        v = 8'h00;
        s = 1'b0;
        r = 0.25;
        $dumpfile("trace.fst");
`ifdef LIMIT
        $dumplimit(`LIMIT);
`endif
        $dumpvars(0, tb);
        v = 8'h01;
        #1 v = 8'h02;
        v = 8'h03;
        s = 1'b1;
        #1 $dumpoff;
        v = 8'h04;
        #1 r = 0.5;
        #1 $dumpon;
        v = 8'h05;
        #1 $dumpflush;
        repeat (200) #1 v = v + 8'h01;
        $strobe("%0t v=%h s=%b r=%.2f", $time, v, s, r);
        #1 $finish;
    end
endmodule
