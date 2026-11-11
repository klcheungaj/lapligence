// Decision S40-D8: real, shortreal, string and chandle leaves of unpacked
// structures and arrays cross the DPI in the C compiler's layout with the C
// types of Table H.1, a string leaf as const char*. An output aggregate
// arrives with every byte zero and each string leaf pointing at an empty
// string. A string leaf the C function gives a new pointer is copied when the
// function returns; every such string is read before any leaf is replaced, so
// one leaf may take the characters another had at the call; a leaf whose
// pointer is unchanged keeps its string.
//
// IEEE 1800-2009 H.7.8 (SystemVerilog-1800-2009.txt L72330-72332):
//   "In the case of an unpacked type that consists purely of unpacked
//   elements (including subaggregates), the layout presented to the C
//   programmer is guaranteed to be compatible with the C compiler's layout on
//   the given operating system."
// IEEE 1800-2009 H.8.10.1 (L72520-72522):
//   "When strings are contained in aggregate arguments, those string members
//   shall also be represented by const char* variables. All the same
//   stipulations apply to string members of aggregate arguments as apply to
//   standalone string arguments."
// IEEE 1800-2009 H.8.10 (L72486-72488):
//   "If the pointer value is modified by the C application, SystemVerilog
//   copies the indicated string contents into its memory space and undertakes
//   any actions sensitive to this change."
// IEEE 1800-2009 H.7.3 (L72168-72169, L72181-72182):
//   "The natural order of elements for each dimension in the layout of an
//   unpacked array shall be used, i.e., elements with lower indices go
//   first. For SystemVerilog range [L:R], the element with SystemVerilog
//   index min(L,R) has the C index 0 and the element with SystemVerilog
//   index max(L,R) has the C index abs(L-R)."
// The initial value of an output is left open (35.5.1.2; decision S40-D1),
// and the text does not order the copies of several changed string leaves:
// llg choices.
//
// Build S40-D8_native_aggregate_leaves.c into a shared library and load it
// with the simulator's DPI library option.
module tb;
    typedef struct { real r; int i; string s; shortreal f; byte b; } leaf_t;
    import "DPI-C" function void d8_leaf(input leaf_t i, output leaf_t o, inout leaf_t io);
    import "DPI-C" function void d8_strings(input string a [2], output string o [2],
        inout string io [3]);
    import "DPI-C" function void d8_reals(input real a [3], inout real io [2]);

    leaf_t li, lo, lio;
    string sa [2];
    string so [2];
    string sio [3];
    real ra [3];
    real rio [2];

    initial begin
        li.r = 1.5; li.i = -7; li.s = "in"; li.f = 2.25; li.b = 8'sd9;
        lo.r = 9.0; lo.i = 9; lo.s = "old"; lo.f = 9.0; lo.b = 8'sd9;
        lio.r = -3.25; lio.i = 42; lio.s = "io"; lio.f = 0.5; lio.b = -8'sd3;
        d8_leaf(li, lo, lio);
        $display("lo=%g %0d %s %g %0d", lo.r, lo.i, lo.s, lo.f, lo.b);
        $display("lio=%g %0d %s %g %0d", lio.r, lio.i, lio.s, lio.f, lio.b);
        sa[0] = "s0"; sa[1] = "s1"; so[0] = "old0"; so[1] = "old1";
        sio[0] = "a"; sio[1] = "b"; sio[2] = "c";
        d8_strings(sa, so, sio);
        $display("so=%s %s sio=%s %s %s", so[0], so[1], sio[0], sio[1], sio[2]);
        ra[0] = 1.25; ra[1] = 2.5; ra[2] = 3.75; rio[0] = 10.0; rio[1] = 20.0;
        d8_reals(ra, rio);
        $display("rio=%g %g", rio[0], rio[1]);
        $finish;
    end
endmodule
