// llg-test-fixture: tests/fixtures/sim/waveform/controls.sv
// LRM: IEEE 1364-2001 18.1.1–18.1.6.
`timescale 1ns/1ps
module wave_child;
    reg \a.b ;
    initial \a.b = 1'b0;
endmodule

module tb;
    reg [3:0] value;
    real analog;
    reg \a@b ;
    wave_child u();

    initial begin
        $dumpfile("trace.vcd");
        value = 4'bxxxx;
        analog = 1.25;
        \a@b = 1'b1;
        $dumpvars(0, tb);
        $dumpvars(0, tb);
        $dumplimit(1000000);
        #1 value = 4'b10z1;
        analog = 2.5;
        #1 $dumpoff;
        value = 4'b0110;
        #1 $dumpon;
        value = 4'b1100;
        #1 $dumpall;
        $dumpflush;
        #1 $finish(0);
    end

    final begin
        value = 4'b0011;
    end
endmodule
