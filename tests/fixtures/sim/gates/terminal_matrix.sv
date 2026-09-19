// llg-test-fixture: gate terminal matrix with hierarchical, selected, and
// constant inputs plus a multi-output buf. LRM: IEEE 1364-1995 7.1-7.4,
// IEEE 1800-2009 28.3-28.6.
module src(input wire [3:0] i, output wire [3:0] o);
    assign o = i;
endmodule

module tb;
    reg [3:0] drv;
    reg [3:0] sel;
    wire [3:0] h;
    wire y_and, y_const, y_hier, y_sel;
    wire [1:0] y_buf;

    src u(.i(drv), .o(h));

    and g1(y_and, sel[2], h[0]);
    and g2(y_const, 1'b1, h[1]);
    and g3(y_hier, h[2], h[3]);
    or g4(y_sel, sel[0], h[3]);
    buf b1(y_buf[0], y_buf[1], h[0]);

    initial begin
        drv = 4'b1010;
        sel = 4'b0110;
        #1;
        $display("CHECK: %b %b %b %b %b %b", y_and, y_const, y_hier, y_sel, y_buf[1], y_buf[0]);
        drv = 4'b0101;
        sel = 4'b0000;
        #1;
        $display("CHECK: %b %b %b %b %b %b", y_and, y_const, y_hier, y_sel, y_buf[1], y_buf[0]);
        $finish(0);
    end
endmodule
