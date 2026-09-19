// llg-test-fixture: IEEE 1800-2009 23.2.2. A fixed-array port whose actual
// selects a higher-rank array element with a runtime index is not a fixed
// connection: the sub-array being connected must be known at elaboration.
// Retained lowering boundary (the analogous `.cnt(cnts[i])` rule).
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
        i = 0;
        #1 $display("%h %h", dst[0], dst[1]);
        $finish(0);
    end
endmodule
