// Decision S42-D6: vpiDecStrVal of a value with unknown or high-impedance
// bits follows the %d display rules: x when all bits are x, z when all are z,
// X when some bits are x, otherwise Z when some are z. A signed value prints
// with a minus sign.
//
// IEEE 1800-2009 38.15 Table 38-3 (SystemVerilog-1800-2009.txt L62649):
//   "vpiDecStrVal  str  String of decimal character(s) [0-9]"
// The table is silent on x and z for decimal strings (it lists x/X/z/Z only
// for octal and hex). llg applies 21.2.1.4 (L36224-36230):
//   "If all bits are at the unknown value, a single lowercase x character is
//   displayed. If all bits are at the high-impedance value, a single
//   lowercase z character is displayed. If some, but not all, bits are at the
//   unknown value, the uppercase X character is displayed. If some, but not
//   all, bits are at the high-impedance value, the uppercase Z character is
//   displayed, unless there are also some bits at the unknown value, in which
//   case the uppercase X character is displayed."
//
// Build S42-D6_decimal_string_xz.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d6()` declared as a system task).
module tb;
    logic [3:0] a = 4'bxxxx;
    logic [3:0] b = 4'b1x01;
    logic [3:0] c = 4'bzzzz;
    logic [3:0] e = 4'b10z1;
    logic [3:0] m = 4'bx0z1;
    logic signed [7:0] s = -8'sd5;
    initial begin
        $d6;
        $display("%%d: a=%0d b=%0d c=%0d e=%0d m=%0d s=%0d", a, b, c, e, m, s);
        $finish;
    end
endmodule
