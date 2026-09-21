// llg-test-fixture: IEEE 1800-2009 23.2.2 and 23.3.3. A fixed-array input
// port can copy a runtime-selected row by value. The selected row is captured
// once per link evaluation, and changing either the selector or a source leaf
// updates the child without changing sibling rows.
module child(input logic [7:0] a [0:1], output logic [7:0] y [0:1]);
    assign y[0] = a[0];
    assign y[1] = a[1];
endmodule

module tb;
    logic [7:0] src [0:1][0:1];
    logic [7:0] dst [0:1];
    int i;
    child u(.a(src[i]), .y(dst));

    initial begin
        src[0][0] = 8'h10;
        src[0][1] = 8'h20;
        src[1][0] = 8'ha0;
        src[1][1] = 8'hb0;
        i = 0;
        #1 $display("%h %h", dst[0], dst[1]);
        i = 1;
        #1 $display("%h %h", dst[0], dst[1]);
        src[1][1] = 8'hc0;
        #1 $display("%h %h", dst[0], dst[1]);
        $finish(0);
    end
endmodule
