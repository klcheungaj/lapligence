// llg-test-fixture: tests/fixtures/sim/waveform/cli_wave.sv
// `llg --wave` dumps this design, which has no waveform tasks.
`timescale 1ns/1ps
module cli_leaf;
    reg [1:0] leaf_value;
    initial begin
        leaf_value = 2'd1;
        #2 leaf_value = 2'd2;
    end
endmodule

module cli_child;
    reg child_value;
    cli_leaf leaf();
    initial begin
        child_value = 1'b0;
        #1 child_value = 1'b1;
    end
endmodule

module tb;
    reg [3:0] top_value;
    cli_child child();
    initial begin
        top_value = 4'h3;
        #3 top_value = 4'ha;
        #1 $finish(0);
    end
endmodule
