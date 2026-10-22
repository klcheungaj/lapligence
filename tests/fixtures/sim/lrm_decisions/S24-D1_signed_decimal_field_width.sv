// Decision S24-D1: the automatic field of a %d conversion (no field width)
// is as wide as the largest unsigned value of the argument's width,
// ceil(width * log10(2)) characters. A signed argument of 2 to 32 bits takes
// the field of a 32-bit integer, 11 characters; a wider signed argument the
// digits of its width, with no extra column for the sign (a value that needs
// more characters expands the field). A 1-bit argument takes one character.
// X and Z digits are padded the same way. An explicit field width, including
// %0d, is kept for known values.
//
// IEEE 1800-2009 21.2.1.3 (SystemVerilog-1800-2009.txt L36149-36156,
// L36184-36187):
//   "For expression arguments, the values written to the output file (or
//   terminal) are sized automatically."
//   "... the result of a 12-bit expression would be allocated three
//   characters when displayed in hexadecimal format and four characters when
//   displayed in decimal format because the largest possible value for the
//   expression is FFF (hexadecimal) and 4095 (decimal)."
//   "If the value is wider than the field width, the field is expanded to
//   contain the converted result."
//
// The text sizes a field by the largest possible value and does not say how
// the sign of a signed value counts. llg follows industry-standard behaviour,
// which sizes signed values of up to 32 bits like a 32-bit integer.
module tb;
  logic signed [0:0] s1;
  logic signed [3:0] s4;
  logic [3:0] u4;
  byte b;
  shortint sh;
  integer i;
  logic signed [32:0] s33;
  logic signed [47:0] s48;
  longint l;
  logic signed [64:0] s65;
  logic signed [15:0] p;
  string text;
  initial begin
    s1 = 1'b1; s4 = -3; u4 = 3; b = -128; sh = 1000; i = -5;
    s33 = -1; s48 = 48'sh7fff_ffff_ffff; l = -5; s65 = 2;
    $display("1-bit :%d: 4-bit :%d: unsigned :%d:", s1, s4, u4);
    $display("byte :%d: shortint :%d: integer :%d:", b, sh, i);
    $display("33-bit :%d: 48-bit :%d:", s33, s48);
    $display("longint :%d: 65-bit :%d:", l, s65);
    $display("expression :%d: :%d:", s4 + 4'sd1, b * 2);
    $display("constant :%d: :%d:", 4'sd3, 2'sb10);
    s4 = 'x; s48 = 'z;
    $display("unknown :%d: :%d:", s4, s48);
    p = 16'b0000_0000_0000_x000;
    $display("partial :%d:", p);
    s4 = -3;
    $display("explicit :%5d: :%2d: :%0d:", s4, sh, s4);
    text = $sformatf("%d", b);
    $display("sformatf :%s:", text);
    $finish;
  end
endmodule
