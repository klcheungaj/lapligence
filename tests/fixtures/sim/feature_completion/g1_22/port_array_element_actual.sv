// llg-test-fixture: IEEE 1800-2009 23.2.2. A fixed-array port actual that
// selects a leading element of a higher-rank array (`src[0]`) connects the
// remaining sub-array by value in both directions; sibling elements stay
// untouched and a later change to the source re-propagates.
module child(
    input  logic [7:0] a [0:1],
    output logic [7:0] y [0:1]
);
    assign y[0] = a[0] + 8'd1;
    assign y[1] = a[1] + 8'd2;
endmodule

module tb;
    logic [7:0] src [0:1][0:1];
    logic [7:0] dst [0:1][0:1];
    child u(.a(src[0]), .y(dst[1]));

    initial begin
        src[0][0] = 8'h10;
        src[0][1] = 8'h20;
        src[1][0] = 8'ha0;
        src[1][1] = 8'hb0;
        #1 $display("d=%h %h", dst[1][0], dst[1][1]);
        src[0][0] = 8'h30;
        #1 $display("d=%h %h", dst[1][0], dst[1][1]);
        $finish(0);
    end
endmodule
