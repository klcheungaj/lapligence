// Decision S42-D9: the value of a real variable is read with vpiRealVal,
// vpiIntVal (rounded to the nearest integer, ties away from zero, as a real to
// integer conversion) or vpiObjTypeVal (which selects vpiRealVal). The binary,
// octal, decimal and hex string formats, vpiStringVal and vpiVectorVal give no
// error: they read the real truncated toward zero as a signed 64-bit value
// (65.75 reads as 65). vpiScalarVal, vpiTimeVal and vpiStrengthVal are errors.
//
// IEEE 1800-2009 38.15 (SystemVerilog-1800-2009.txt L62562-62565):
//   "When the format field is vpiObjTypeVal, the routine shall fill in the
//   value and change the format field based on the object type ... For a
//   real, vpiRealVal"
// Table 38-3 (L62645-62667) does not say how a real converts to the string,
// scalar or vector formats; the text is silent. The vpiIntVal rounding
// follows 6.12.2 (L5531-5533); the truncation of the other formats and the
// 64-bit width (vpiSize of a real, S42-D8) are implementation-defined.
//
// Build S42-D9_real_value_formats.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d9()` declared as a system task).
module tb;
    real r = -2.5;
    real p = 65.75;
    initial begin
        $d9;
        $finish;
    end
endmodule
