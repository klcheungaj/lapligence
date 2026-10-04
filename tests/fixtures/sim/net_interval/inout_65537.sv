// llg-test-fixture: KI-NET-INTERVAL whole net-array inout, 65,537 cells
// IEEE 1800-2009 23.3.3.7: one connected network per cell; only two cells
// have drivers, so generated code must not grow with the cell count.
`timescale 1ns/1ns
module child(inout wire [7:0] c [0:65536], input logic en);
    assign c[1] = en ? 8'h5a : 8'hzz;
endmodule
module tb;
    wire [7:0] n [0:65536];
    logic en, pen;
    integer i;
    child u(.c(n), .en(en));
    assign n[0] = pen ? 8'h3c : 8'hzz;
    initial begin
        en = 0; pen = 1; i = 40000;
        #1 $display("%h %h %h %h %h", n[0], n[1], n[65536], n[i], u.c[0]);
        en = 1; pen = 0;
        #1 $display("%h %h %h %h", n[0], n[1], u.c[1], u.c[65536]);
        en = 1; pen = 1;
        #1 $display("%b %b", n[0], n[1]);
        $finish(0);
    end
endmodule
