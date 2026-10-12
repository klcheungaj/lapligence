// SIM-040 A03: every svdpi.h routine llg provides (SV H.10.1, H.11, H.12),
// with in-range, boundary and invalid arguments. Invalid dimensions and
// indices follow decisions S40-D3 and S40-D4. Build svdpi_access.c into a shared
// library and pass it with --dpi-lib.
module tb;
    import "DPI-C" function void sa_version();
    import "DPI-C" function void sa_select(inout logic [39:0] l, inout bit [39:0] b);
    import "DPI-C" function void sa_query(input int c [][][], input bit [] p, input logic s []);
    import "DPI-C" function void sa_pointers(inout int c [][][]);
    import "DPI-C" function void sa_scalars(inout logic l1 [], inout logic l2 [][],
                                            inout logic l3 [][][], inout bit b1 [],
                                            inout bit b2 [][], inout bit b3 [][][]);
    import "DPI-C" function void sa_vectors(inout logic [9:0] l1 [], inout logic [9:0] l2 [][],
                                            inout logic [9:0] l3 [][][],
                                            inout bit [35:0] b1 [], inout bit [35:0] b2 [][],
                                            inout bit [35:0] b3 [][][]);

    logic [39:0] sl;
    bit [39:0] sb;
    int c [1:0][2:4][-1:-2];
    bit [11:4] p;
    logic s [3];
    logic l1 [2:0];
    logic l2 [0:1][1:0];
    logic l3 [1:0][0:0][2:3];
    bit b1 [0:2];
    bit b2 [1:0][0:1];
    bit b3 [0:1][0:0][3:2];
    logic [9:0] v1 [1:2];
    logic [9:0] v2 [1:0][0:1];
    logic [9:0] v3 [0:0][1:0][0:1];
    bit [35:0] w1 [2:1];
    bit [35:0] w2 [0:1][1:0];
    bit [35:0] w3 [1:1][0:1][1:0];

    initial begin
        sa_version();

        sl = 40'hzx_0123_4567;
        sb = 40'h80_0000_0001;
        sa_select(sl, sb);
        $display("select sl=%h sb=%h", sl, sb);

        p = 8'ha5;
        s = '{1'b1, 1'bx, 1'bz};
        sa_query(c, p, s);

        foreach (c[i, j, k]) c[i][j][k] = i * 100 + j * 10 - k;
        sa_pointers(c);
        foreach (c[i, j, k]) if (c[i][j][k] < 0) $display("c[%0d][%0d][%0d]=%0d", i, j, k, c[i][j][k]);

        l1 = '{1'b0, 1'b1, 1'bx};
        l2 = '{'{1'bz, 1'b0}, '{1'b1, 1'bx}};
        l3 = '{'{'{1'b0, 1'b1}}, '{'{1'bx, 1'bz}}};
        b1 = '{1'b1, 1'b0, 1'b1};
        b2 = '{'{1'b0, 1'b1}, '{1'b1, 1'b0}};
        b3 = '{'{'{1'b1, 1'b1}}, '{'{1'b0, 1'b0}}};
        sa_scalars(l1, l2, l3, b1, b2, b3);
        $display("l1=%b%b%b l2=%b%b%b%b l3=%b%b%b%b", l1[2], l1[1], l1[0], l2[0][1], l2[0][0],
                 l2[1][1], l2[1][0], l3[1][0][2], l3[1][0][3], l3[0][0][2], l3[0][0][3]);
        $display("b1=%b%b%b b2=%b%b%b%b b3=%b%b%b%b", b1[0], b1[1], b1[2], b2[1][0], b2[1][1],
                 b2[0][0], b2[0][1], b3[0][0][3], b3[0][0][2], b3[1][0][3], b3[1][0][2]);

        v1 = '{10'h3ff, 10'b10_xz01_zx10};
        foreach (v2[i, j]) v2[i][j] = 10'(i * 16 + j);
        foreach (v3[i, j, k]) v3[i][j][k] = 10'(j * 2 + k);
        v3[0][1][1] = 'x;
        w1 = '{36'h8_0000_0001, 36'h1_2345_6789};
        foreach (w2[i, j]) w2[i][j] = 36'(i * 2 + j) << 32;
        foreach (w3[i, j, k]) w3[i][j][k] = 36'(j * 2 + k);
        sa_vectors(v1, v2, v3, w1, w2, w3);
        $display("v1=%b,%b v2=%h,%h,%h,%h v3=%h,%h,%h,%h", v1[1], v1[2], v2[1][0], v2[1][1],
                 v2[0][0], v2[0][1], v3[0][1][0], v3[0][1][1], v3[0][0][0], v3[0][0][1]);
        $display("w1=%h,%h w2=%h,%h,%h,%h w3=%h,%h,%h,%h", w1[2], w1[1], w2[0][1], w2[0][0],
                 w2[1][1], w2[1][0], w3[1][0][1], w3[1][0][0], w3[1][1][1], w3[1][1][0]);
        $finish;
    end
endmodule
