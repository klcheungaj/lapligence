// SIM-040 A01: unpacked structures and sized unpacked arrays cross the DPI
// with the C compiler's layout (SV H.7.8, H.11.4) in natural index order
// (H.7.6), packed members in canonical form (H.10.3); strings pass as C
// strings (H.8.10). Build aggregates.c into a shared library and pass it with
// --dpi-lib.
module tb;
    typedef struct { int id; logic [3:0] tag; bit [39:0] wide; byte pair [2]; } rec_t;
    typedef struct { rec_t r [2:1]; shortint n; } nest_t;

    import "DPI-C" function void ag_rec(input rec_t i, output rec_t o, inout nest_t n);
    import "DPI-C" function void ag_arrays(input byte b [3:0],
        output logic [3:0] o [0:1][2:0], input bit f [4], inout logic l [1:3]);
    import "DPI-C" function string ag_strings(input string i, output string o,
        inout string io);

    rec_t r;
    rec_t ro;
    nest_t n;
    byte b [3:0];
    logic [3:0] o [0:1][2:0];
    bit f [4];
    logic l [1:3];
    string so;
    string sio;
    string sret;

    initial begin
        r.id = -7;
        r.tag = 4'b1x0z;
        r.wide = 40'hab_cdef_0123;
        r.pair[0] = 8'sd1;
        r.pair[1] = -8'sd2;
        n.r[2] = r;
        n.r[1].id = 9;
        n.r[1].tag = 4'h5;
        n.r[1].wide = 40'h1;
        n.r[1].pair[0] = 8'sd3;
        n.r[1].pair[1] = 8'sd4;
        n.n = -16'sd300;
        ag_rec(r, ro, n);
        $display("ro=%0d %b %h %0d %0d", ro.id, ro.tag, ro.wide, ro.pair[0], ro.pair[1]);
        $display("n.r[2]=%0d %b n.r[1]=%0d %b %h n=%0d", n.r[2].id, n.r[2].tag,
            n.r[1].id, n.r[1].tag, n.r[1].wide, n.n);
        foreach (b[i]) b[i] = 8'(i * 3 - 4);
        f[0] = 1'b1;
        f[1] = 1'b0;
        f[2] = 1'b0;
        f[3] = 1'b1;
        l[1] = 1'b0;
        l[2] = 1'bx;
        l[3] = 1'bz;
        ag_arrays(b, o, f, l);
        foreach (o[i, j]) $display("o[%0d][%0d]=%b", i, j, o[i][j]);
        $display("l=%b%b%b", l[1], l[2], l[3]);
        sio = "inout";
        sret = ag_strings("input", so, sio);
        $display("so=%s sio=%s sret=%s", so, sio, sret);
        $finish;
    end
endmodule
