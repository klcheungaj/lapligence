// llg-test-fixture: ADV-020 value arrays. value_array.c ($va_get, $va_put)
// reads and writes sections of static unpacked arrays with
// vpi_get_value_array and vpi_put_value_array (IEEE 1800-2009 38.16,
// 38.35) in every format, with and without vpiUserAllocFlag, vpiOneValue
// and vpiPropagateOff, and checks the errors. Expected output:
// value_array.out.
module tb;
    logic [41:0] a [2:0][3:5];
    bit [11:0] b [0:3];
    byte by [0:2];
    shortint si [0:1];
    int iv [1:0];
    longint li [0:1];
    real r [0:2];
    shortreal sr [0:1];
    integer ig [0:1];
    time t [0:1];
    wire [3:0] n [0:1];
    typedef struct { int x; int y; } pair_t;
    pair_t us [0:1];
    logic [7:0] w;
    int changes = 0;

    assign n[0] = 4'ha;
    assign n[1] = 4'bxz01;
    always @(b[1]) changes++;

    initial begin
        for (int i = 2; i >= 0; i--)
            for (int j = 3; j <= 5; j++)
                a[i][j] = 42'h3_0000_0000_00 | (i * 16 + j);
        a[0][5] = 42'hx;
        a[1][5][7:4] = 4'bz1x0;
        b[0] = 12'h801; b[1] = 12'h0ff; b[2] = 12'h123; b[3] = 12'hfff;
        by[0] = -2; by[1] = 8'sd127; by[2] = -128;
        si[0] = -300; si[1] = 32767;
        iv[1] = -5; iv[0] = 70000;
        li[0] = -64'sd1234567890123; li[1] = 64'sh7fff_ffff_ffff_fffe;
        r[0] = 1.5; r[1] = -2.25; r[2] = 1e10;
        sr[0] = 0.5; sr[1] = -3.0;
        ig[0] = -7; ig[1] = 'x;
        t[0] = 64'd5000000000; t[1] = 3;
        us[0].x = 1;
        w = 8'h5a;
        #1 $va_get;
        changes = 0;
        $va_put;
        #1;
        $display("a[1][4]=%h a[1][5]=%h a[0][3]=%h a[0][4]=%h a[0][5]=%h",
                 a[1][4], a[1][5], a[0][3], a[0][4], a[0][5]);
        $display("a[2][3]=%h a[2][5]=%h", a[2][3], a[2][5]);
        $display("b=%h %h %h %h changes=%0d", b[0], b[1], b[2], b[3], changes);
        $display("by=%0d %0d %0d si=%0d %0d iv[1]=%0d iv[0]=%0d", by[0], by[1], by[2],
                 si[0], si[1], iv[1], iv[0]);
        $display("li=%0d %0d r=%g %g %g sr=%g %g", li[0], li[1], r[0], r[1], r[2], sr[0], sr[1]);
        $display("ig=%0d %h t=%0d %0d", ig[0], ig[1], t[0], t[1]);
        $finish;
    end
endmodule
