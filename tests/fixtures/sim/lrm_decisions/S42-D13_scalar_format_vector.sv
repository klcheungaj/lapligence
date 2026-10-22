// Decision S42-D13: vpiScalarVal on a vector reads its least significant bit
// (v = 0101 reads vpi1) and writes the scalar zero-extended, as an assignment
// of a one-bit unsigned value (vpiX gives 000x). A scalar or a bit-select
// takes it as one bit; vpiH and vpiL write 1 and 0.
//
// IEEE 1800-2009 38.15 Table 38-3 (SystemVerilog-1800-2009.txt L62657):
//   "vpiScalarVal  scalar  vpi1, vpi0, vpiX, vpiZ, vpiH, vpiL"
// and L62567 ("For a scalar, either vpiScalar or vpiStrength") tie the
// format to scalars; what it means for a vector is not specified, so the
// vector rule is implementation-defined. The text does not define vpiH and
// vpiL (Annex K L75684-75685 only gives their codes 4 and 5); llg reads them
// as the high and low levels and writes them as 1 and 0.
//
// Build S42-D13_scalar_format_vector.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d13()` declared as a system task).
module tb;
    logic [3:0] v = 4'b0101;
    logic s = 1'b0;
    initial begin
        $d13;
        $display("v=%b s=%b", v, s);
        $finish;
    end
endmodule
