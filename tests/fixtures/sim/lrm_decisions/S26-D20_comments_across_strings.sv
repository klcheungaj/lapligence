// Decision S26-D20 (llg choice): a // comment ends with the string that holds
// it, and a /* comment still open at the end of a string continues in the
// next string.
//
// IEEE 1800-2009 D.14 (SystemVerilog-1800-2009.txt L70159-70160): the strings
// "take the same format as the strings that appear in the input files". 21.4
// (L37268-37271): the text may contain "Comments (both types of comment are
// allowed)". The text does not say how a comment relates to the end of a
// string; llg reads the strings as one image with a newline after each.
module tb;
  logic [7:0] m [0:3];
  initial begin
    m = '{default: 8'h5a};
    $sreadmemh(m, 0, 1, "11 // 99", "22");
    $display("a %h %h %h", m[0], m[1], m[2]);
    m = '{default: 8'h5a};
    $sreadmemh(m, 0, 1, "11 /* 99", "88 */ 22");
    $display("b %h %h %h", m[0], m[1], m[2]);
    $finish;
  end
endmodule
