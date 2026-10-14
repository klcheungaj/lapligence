// SIM-027 Q02 resolution: a memory-file word is an unsized number of the
// task's radix, written "as in a ... source description" without length or
// base (IEEE 1364-2001 17.2.8; IEEE 1800-2009 21.4). Assigned to a word it
// behaves like that literal (IEEE 1364-2001 3.5.1; IEEE 1800-2009 5.7.1):
// a leftmost x/z digit fills the upper bits with x/z, a leftmost known digit
// fills with zeros, even for a signed word, because the number is unsigned;
// a wider number loses its upper bits. Expected values are derived by hand.
module tb;
  reg [7:0] hex8 [0:4];
  reg [11:0] hex12 [0:4];
  reg signed [3:0] hex4s [0:4];
  reg [7:0] bin8 [0:4];
  reg signed [0:0] bin1s [0:4];
  initial begin
    $readmemh("tokens.hex", hex8, 0, 4);
    $readmemh("tokens.hex", hex12, 0, 4);
    $readmemh("tokens.hex", hex4s, 0, 4);
    $readmemb("tokens.bin", bin8, 0, 4);
    $readmemb("tokens.bin", bin1s, 0, 4);
    $display("hex8 %b %b %b %b %b", hex8[0], hex8[1], hex8[2], hex8[3], hex8[4]);
    $display("hex12 %b %b %b %b %b", hex12[0], hex12[1], hex12[2], hex12[3], hex12[4]);
    $display("hex4s %b %b %b %b %b", hex4s[0], hex4s[1], hex4s[2], hex4s[3], hex4s[4]);
    $display("bin8 %b %b %b %b %b", bin8[0], bin8[1], bin8[2], bin8[3], bin8[4]);
    $display("bin1s %b %b %b %b %b", bin1s[0], bin1s[1], bin1s[2], bin1s[3], bin1s[4]);
    $finish;
  end
endmodule
