// SIM-024: integral conversions, automatic field sizes, explicit widths,
// unknown digits and display argument lists (SV 21.2.1).
module tb;
  logic [11:0] r1;
  logic signed [7:0] s8;
  logic [3:0] xz;
  logic [65:0] w66;
  int i;
  integer ix;
  byte b;
  initial begin
    r1 = 12'd10;
    s8 = -8'sd5;
    xz = 4'b1x0z;
    w66 = {2'b10, 64'hffff_ffff_ffff_fffe};
    i = 5;
    ix = 'x;
    b = -8'sd128;
    $display("A|%d|%h|%o|%b|", r1, r1, r1, r1);
    $display("B|%0d|%0h|%0o|%0b|", r1, r1, r1, r1);
    $display("C|%d|%0d|%5d|%-5d|%05d|%2d|", i, i, i, i, i, 123);
    $display("D|%d|%0d|%h|%b|", s8, s8, s8, s8);
    $display("E|%d|%d|%h|%o|%b|", xz, ix, xz, xz, xz);
    $display("F|%h|%0d|", w66, w66);
    $display("G|%3h|%3h|%8b|%10h|", 32'h5, 32'h1234, 4'b0101, 8'hab);
    $display("H|%-6h|%-4d|", 8'hab, 7);
    $display("I|%c%c|%s|%5s|%-5s|", 8'h41, 16'h4243, 16'h4142, 16'h4142, 16'h4142);
    $display("J|%d|%h|", b, b);
    $display("K|%e|%f|%.2f|", 3, i, 2'b10);
    $display("L|%d|%h|", 2.5, -1.5);
    $display(r1, " and ", i);
    $display("M", , "N");
    $display("P", r1, s8);
    $displayh(r1, " ", s8);
    $displayb(4'b1010);
    $displayo(9'o17);
    $write("Q|%d|", 3'd5);
    $write("%s\n", "end");
    $display("R|%s|%d|%%|", "x", "AB");
    $finish(0);
  end
endmodule
