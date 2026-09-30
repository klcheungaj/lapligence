// llg-test-fixture: tests/fixtures/sim/partial_features/udp_masks.sv
// IEEE 1800-2009 29.3.4-29.4: legal overlapping rows agree on output.
primitive udp_masks(y, a, b);
    output y;
    input a, b;
    table
        b 0 : 1;
        b ? : 1;
        x 0 : 0;
        x x : x;
    endtable
endprimitive

module tb;
    reg a, b;
    reg [3:0] va, vb;
    wire y;
    wire [3:0] array_y;
    udp_masks scalar(y, a, b);
    udp_masks array[3:0](array_y, va, vb);

    initial begin
        a = 0; b = 0;
        #1 $display("00 %b", y);
        b = 1;
        #1 $display("01 %b", y);
        b = 1'bx;
        #1 $display("0x %b", y);
        b = 1'bz;
        #1 $display("0z %b", y);
        a = 1; b = 0;
        #1 $display("10 %b", y);
        b = 1;
        #1 $display("11 %b", y);
        b = 1'bx;
        #1 $display("1x %b", y);
        b = 1'bz;
        #1 $display("1z %b", y);
        a = 1'bx; b = 0;
        #1 $display("x0 %b", y);
        b = 1;
        #1 $display("x1 %b", y);
        b = 1'bx;
        #1 $display("xx %b", y);
        b = 1'bz;
        #1 $display("xz %b", y);
        a = 1'bz; b = 0;
        #1 $display("z0 %b", y);
        b = 1;
        #1 $display("z1 %b", y);
        b = 1'bx;
        #1 $display("zx %b", y);
        b = 1'bz;
        #1 $display("zz %b", y);
        va = 4'b10xz; vb = 4'bz001;
        #1 $display("array %b", array_y);
        $finish(0);
    end
endmodule
