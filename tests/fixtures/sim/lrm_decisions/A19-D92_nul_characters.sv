// A19-D92: 38.15 (SystemVerilog-1800-2009.txt L62664-62665): "A string where
// each 8-bit group of the value of the object is assumed to represent an
// ASCII character". 5.9.1 allows a NUL character in a string literal, which a
// vpiStringVal C string cannot hold. Decision: a group that is NUL (its x and
// z bits read as 0) prints as a space; a vector of at most 8 bits that is NUL
// prints as no character; the vector formats give the exact value.
module tb;
  reg [63:0] v, w, u;
  reg [15:0] h;
  reg [10:0] odd;
  reg [8:0] nine;
  reg [7:0] c, cx;
  reg a;
  wire [23:0] n = {8'h41, 8'h00, 8'h42};
  string s;
  parameter P = "p\000q";
  parameter [47:0] Q = "\000xy";
  initial begin
    v = "a\000b";
    s = "\000\000lead";
    s = "tail\000";
    c = "";
    c = "\000";
    v = {"a\000b", 8'h41};
    s = "quote\" back\\ line\n";
  end
  initial begin
    #1 v = "ab";
    w = 64'h4100_4200_0000_4300;
    u = 64'h41xx_42_0000_zz_00_43;
    h = 16'h4100;
    odd = 11'h141;
    nine = 9'h1xx;
    c = 0;
    cx = 8'h4x;
    a = 0;
    $d92("args", v, w, u, h, odd, nine, c, cx, a, n, v[7:0], v[23:16], {"a", 8'h0, "b"});
    $d92("literals", "a\000b", "\000", "", "\000\000", "\000a", "\001\000", P, Q);
    #1 $finish;
  end
endmodule
