// Decision A19-D4: typespec identity. A typespec belongs to its
// declaration: the vpiTypespec handles of two declarations are different
// objects (vpi_compare_objects returns 0) even when both use one typedef or
// the same built-in type and ranges; two vpiTypespec handles of one
// declaration are the same object (1). A struct, union or enum type is the
// exception: it is one object shared by every declaration of that type.
//
// IEEE 1800-2009 38.3 (SystemVerilog-1800-2009.txt L61867-61868):
//   "The VPI routine vpi_compare_objects() shall return 1 (TRUE) if the two
//    handles refer to the same underlying simulation object at the time the
//    function is called, provided that the simulation object exists."
// 37.23 does not say whether a typespec is one object per type or one per
// declaration; this is an llg choice following industry-standard behaviour.
//
// Build A19-D4_typespec_identity.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d4()` declared as a system task).
module tb;
    typedef logic [3:0] nib_t;
    typedef logic [3:0] other_t;
    nib_t n1, n2;
    other_t o;
    logic [3:0] x, y;
    logic [4:0] z;
    initial begin
        $d4;
        $finish;
    end
endmodule
