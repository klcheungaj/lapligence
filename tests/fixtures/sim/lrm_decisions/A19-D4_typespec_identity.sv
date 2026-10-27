// Decision A19-D4: typespec identity. Two typespec handles are the same
// object (vpi_compare_objects returns 1) when they describe the same type
// with the same name: variables declared with one typedef share its
// typespec, and two declarations written with the same built-in type and
// ranges share one unnamed typespec. Typespecs that differ in name (a
// typedef and the same type written inline, or two typedefs of one type)
// are different objects.
//
// IEEE 1800-2009 38.3 (SystemVerilog-1800-2009.txt L61867-61868):
//   "The VPI routine vpi_compare_objects() shall return 1 (TRUE) if the two
//    handles refer to the same underlying simulation object at the time the
//    function is called, provided that the simulation object exists."
// 37.23 does not say whether a typespec is one object per type or one per
// declaration; this is an llg choice.
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
