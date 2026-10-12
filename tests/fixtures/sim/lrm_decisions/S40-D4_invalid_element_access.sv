// Decision S40-D4: open-array element access with an index outside its
// dimension's range, or with a different number of indices than the array
// has unpacked dimensions, does not touch the array: svGetArrElemPtr*
// returns NULL, svGet*ArrElem* reads the element type's default (X for
// logic, 0 for bit) and svPut*ArrElem* does nothing. Reading a logic array
// through the svBit accessors maps X and Z to 0.
//
// IEEE 1800-2009 H.12.4 (SystemVerilog-1800-2009.txt L73337-73340):
//   "If the actual layout of the SystemVerilog array passed as an argument for
//   an open unpacked array is different from the C layout, then it is not
//   possible to access such an array as a whole; therefore, the address and
//   size of such an array shall be undefined (0, to be exact). Nonetheless,
//   the addresses of individual elements of an array shall be always
//   supported."
// IEEE 1800-2009 7.4.6 (L8235-8237):
//   "If an index expression is out of the address bounds or if any bit in the
//   address is X or Z, then the index shall be invalid. The result of reading
//   from an array with an invalid index shall return the default uninitialized
//   value for the array element type. Writing to an array with an invalid
//   index shall perform no operation."
// Annex H does not say what an invalid index does in the C layer; llg
// applies 7.4.6 to the svdpi.h accessors (llg choice).
//
// Build S40-D4_invalid_element_access.c into a shared library and load it
// with the simulator's DPI library option.
module tb;
    import "DPI-C" function void d4_access(inout logic [3:0] l [], inout bit b [][]);
    logic [3:0] l [1:2];
    bit b [0:1][3:2];

    initial begin
        l[1] = 4'b1x0z;
        l[2] = 4'b0011;
        b = '{'{1'b1, 1'b0}, '{1'b0, 1'b1}};
        d4_access(l, b);
        $display("l=%b,%b b=%b%b%b%b", l[1], l[2], b[0][3], b[0][2], b[1][3], b[1][2]);
        $finish;
    end
endmodule
