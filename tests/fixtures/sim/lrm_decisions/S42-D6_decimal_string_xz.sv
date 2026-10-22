// Decision S42-D6: a vpiDecStrVal string holds only decimal digits and a
// sign. A value with any unknown or high-impedance bit reads as 0. The
// string is right-justified with leading spaces in a field of the digits of
// the largest unsigned value of the object's width (a 4-bit value takes 2
// characters), except that a signed object of 2 to 32 bits takes the 11
// characters of a signed 32-bit integer; a constant takes the unsigned field
// plus one character for a minus sign.
//
// IEEE 1800-2009 38.15 Table 38-3 (SystemVerilog-1800-2009.txt L62649):
//   "vpiDecStrVal  str  String of decimal character(s) [0-9]"
// Unlike the octal and hex rows (L62642-62655), the row lists no x/X/z/Z
// characters, so llg gives no x/z digit. The x/z mapping follows the
// vpiIntVal row (L62659-62660): "Any bits x or z in the value of the object
// are mapped to a 0", applied to the whole value. The padding follows the
// automatic sizing of 21.2.1.3 (L36149-36156): "the result of a 12-bit
// expression would be allocated ... four characters when displayed in
// decimal format because the largest possible value for the expression is
// ... 4095 (decimal). When displaying decimal values, leading zeros are
// suppressed and replaced by spaces." The 11-character signed field is
// implementation-defined (the text is silent on signed fields).
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
    logic [9:0] u = 10'd7;
    logic signed [39:0] w = -40'sd5;
    logic signed [0:0] one = 1'sb1;
    initial begin
        $d6;
        $display("%%d: a=%0d b=%0d c=%0d e=%0d m=%0d s=%0d", a, b, c, e, m, s);
        $finish;
    end
endmodule
