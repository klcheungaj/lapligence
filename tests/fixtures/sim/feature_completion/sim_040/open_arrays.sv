// SIM-040 A01: open-array formals (SV 35.5.6.1, H.12) take each call's
// actual ranges, including reversed, nonzero and multidimensional unpacked
// dimensions; an unsized packed dimension is the actual's linearized width
// (H.7.5). Build open_arrays.c into a shared library and pass it with
// --dpi-lib.
module tb;
    typedef struct { byte b; int i; } pair_t;

    import "DPI-C" function int oa_walk(input int a [][]);
    import "DPI-C" function void oa_fill(output int a [][]);
    import "DPI-C" function void oa_cube(inout shortint c [][][]);
    import "DPI-C" function int oa_packed(input bit [] p);
    import "DPI-C" function void oa_vectors(inout logic [] v [], input bit [] p []);
    import "DPI-C" function void oa_sized(input logic [] s [3:1]);
    import "DPI-C" function int oa_records(input pair_t s [], output pair_t d []);

    int m [1:0][3:5];
    int n [2:4][7:6];
    shortint c [1:0][2:3][-1:-1];
    bit [3:0][2:0] p2;
    logic [11:0] v [5:2];
    bit [5:0] p1 [0:1];
    logic [7:0] s [0:2];
    pair_t src [3:5];
    pair_t dst [3:5];
    int sum;

    initial begin
        foreach (m[i, j]) m[i][j] = i * 10 + j;
        $display("walk=%0d", oa_walk(m));
        oa_fill(n);
        foreach (n[i, j]) $display("n[%0d][%0d]=%0d", i, j, n[i][j]);
        foreach (c[i, j, k]) c[i][j][k] = shortint'(i * 100 + j * 10 - k);
        oa_cube(c);
        foreach (c[i, j, k]) $display("c[%0d][%0d][%0d]=%0d", i, j, k, c[i][j][k]);
        p2 = 12'habc;
        $display("packed=%0d", oa_packed(p2));
        $display("packed13=%0d", oa_packed(13'h1abc));
        foreach (v[i]) v[i] = 12'h100 + 12'(i);
        p1[0] = 6'h21;
        p1[1] = 6'h3f;
        oa_vectors(v, p1);
        foreach (v[i]) $display("v[%0d]=%h", i, v[i]);
        s[0] = 8'h10;
        s[1] = 8'h11;
        s[2] = 8'h12;
        oa_sized(s);
        foreach (src[i]) begin
            src[i].b = 8'(i);
            src[i].i = i * 1000;
        end
        sum = oa_records(src, dst);
        $display("records=%0d", sum);
        foreach (dst[i]) $display("dst[%0d]=%0d,%0d", i, dst[i].b, dst[i].i);
        $finish;
    end
endmodule
