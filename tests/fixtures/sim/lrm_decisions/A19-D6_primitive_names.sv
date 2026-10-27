// Decision A19-D6: names of primitive instances. A gate's vpiDefName is its
// gate keyword ("and", "nand", ...) and a UDP's is the primitive's name. A
// primitive instance written without a name has no name: vpiName and
// vpiFullName are NULL with a vpiError status, and vpi_handle_by_name cannot
// find it. An element of a primitive array is named after the array and its
// index (`ga[1]`), is found by that name, and vpiIndex gives the index; the
// elements own the slices of a connection as wide as the array, the least
// significant slice belonging to the right-hand index.
//
// IEEE 1800-2009 37.31 (SystemVerilog-1800-2009.txt L59736-L59786) lists
// "-> definition name str: vpiDefName" (L59760-L59761) and "-> name str:
// vpiName str: vpiFullName" without saying what a gate's definition name is
// or what an unnamed instance reports; detail 4 (L59785-L59786): "If a
// primitive is an element within a primitive array, the vpiIndex transition
// is used to access the index within the array. If a primitive is not part
// of a primitive array, this transition shall return NULL." The names are
// llg choices; the slices follow 28.3.6.
//
// Build A19-D6_primitive_names.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d6()` declared as a system task).
`timescale 1ns/1ns
module tb;
    logic a = 1'b1, b = 1'b0;
    wire y0, y1;
    wire [1:0] ya;
    and (y0, a, b);
    or g_or (y1, a, b);
    nand ga [0:1] (ya, {a, b}, {a, a});
    initial begin
        #1 $d6;
        $display("hdl y0=%b y1=%b ya=%b", y0, y1, ya);
        $finish;
    end
endmodule
