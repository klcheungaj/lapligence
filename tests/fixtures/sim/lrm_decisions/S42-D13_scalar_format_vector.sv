// Decision S42-D13: vpiScalarVal reads and writes only one-bit objects (a
// scalar or a bit-select); used on a vector it is an error and the value is
// left unchanged.
//
// IEEE 1800-2009 38.15 Table 38-3 (SystemVerilog-1800-2009.txt L62657):
//   "vpiScalarVal  scalar  vpi1, vpi0, vpiX, vpiZ, vpiH, vpiL"
// and L62567 ("For a scalar, either vpiScalar or vpiStrength") tie the
// format to scalars; what it means for a vector is not specified. 38.34
// (L64218-64219) makes vpiStrengthVal illegal for a vector put; llg treats
// vpiScalarVal the same way.
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
