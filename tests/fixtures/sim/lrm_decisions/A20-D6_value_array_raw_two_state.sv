// Decision A20-D6: raw two-state values of a four-state array. Reading a
// four-state array with vpiRawTwoStateVal gives each x and z bit as 0 (the A
// bit of a 1 is kept). Writing a two-state array with vpiRawFourStateVal
// ignores the B bytes, and writing a four-state array with vpiRawTwoStateVal
// takes every B bit as 0.
//
// IEEE 1800-2009 38.16 (SystemVerilog-1800-2009.txt L63052-L63054): "The
// vpiRawFourStateVal or vpiVectorVal formats can also be requested of a
// 2-state array type, and the vpiRawTwoStateVal format can be requested for a
// 4-state array type." The text does not say what an x or z bit reads as in
// the two-state form; llg reads them as 0, as a four-state to two-state
// conversion does (6.11.2, L5489-L5490: "When a 4-state value is automatically
// converted to a 2-state value, any unknown or high-impedance bits shall be
// converted to zeros."). 38.35 (L64449-L64450): "If the
// vpiRawFourStateVal format is set for a 2-state array type, the bvalbits
// shall be ignored. If the vpiRawTwoStateVal format is specified for a 4-state
// array type, the bvalbits shall be assumed to be 0."
//
// Build A20-D6_value_array_raw_two_state.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d6()` declared as a system task).
module tb;
    logic [7:0] q [0:1];
    bit [7:0] b [0:0];
    initial begin
        q[0] = 8'b10xz_01zx;
        q[1] = 8'h5a;
        b[0] = 0;
        $d6;
        $display("q=%b %b b=%b", q[0], q[1], b[0]);
        $finish;
    end
endmodule
