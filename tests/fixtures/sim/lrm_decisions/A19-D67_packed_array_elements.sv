// Decision A19-D67: the elements and bits of a packed array are objects.
// vpi_handle_by_index, vpi_handle_by_multi_index and vpi_handle_by_name
// with every packed index but the last give an element (a vpiReg, a vpiBitVar
// for a two-state array, a vpiNet for a net, sized by the last dimension);
// with every packed index they give a bit: a var bit (vpiRegBit) of a
// variable, also of a two-state array, or a vpiNetBit of a net (decision
// A19-D70; 37.17 details 19 and 21, L58786-58808 and L58849). An
// element of an unpacked array that is a packed array takes the packed
// indices after the unpacked ones. Other index counts (a subarray of a
// three-dimensional packed array) and indices outside the declared ranges
// return NULL with an error. An element's vpiParent is the packed array (or
// the unpacked element holding it), vpiIndex its index in a one-dimensional
// packed array, vpiLeftRange/vpiRightRange its range, vpiBit its bits; its
// value reads and writes that part of the array, and vpiArray is FALSE for
// an element and for an element of an unpacked array.
//
// IEEE 1800-2009 38.20 (SystemVerilog-1800-2009.txt L63330-63331,
// L63349-63352): vpi_handle_by_multi_index "shall provide access to an
// index-selected subobject of the reference handle ... The order of the
// indices provided shall follow the array dimension declaration from the
// leftmost range to the rightmost range of the reference handle; the array
// indices may be optionally followed by a bit-select index. If the indices
// provided do not lead to the construction of a legal SystemVerilog index
// select expression, the routine shall return a null handle." The text
// leaves open whether a subarray of a packed array is a subobject; llg, like
// industry-standard behaviour, returns NULL for it. 37.18 detail 6
// (L58962-58965): "vpi_iterate(vpiIndex, packed_array_var_handle) shall
// return the set of indices for a sub-element of a packed array variable
// (relative to its vpiParent), starting with the index for the sub-element
// and working outwards ... The indices will be retrieved in right-to-left
// order as they appear in the text." 37.17 detail 6 (L58754-58755): "The
// vpiLeftRange and vpiRightRange relations shall return the bounds of the
// leftmost packed dimension". 7.4.1 (L8032-8033): "The individual elements
// of the array are unsigned unless they are of a named type declared as
// signed." The
// element types follow industry-standard behaviour; some simulators report
// a bit of a packed array as a vpiBitSelect. Some simulators iterate
// the indices of a bit left to right; llg follows the clause text, innermost
// index first (as decision A19-D28 does for unpacked elements).
//
// Build A19-D67_packed_array_elements.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d67()` declared as a system task).
module tb;
    logic [3:0][7:0] pa;
    bit [1:0][2:0][3:0] pb;
    wire [1:0][3:0] pn;
    logic [1:0][3:0] upa [0:1];
    logic signed [1:0][3:0] ps;
    assign pn = 8'h5a;
    initial begin
        pa = 32'h44332211;
        pb = 24'habcdef;
        upa[0] = 8'h12;
        upa[1] = 8'h34;
        ps = 8'h9c;
        #1 $d67;
        #1 $display("pa=%h pb=%h upa0=%h ps=%h", pa, pb, upa[0], ps);
        $finish;
    end
endmodule
