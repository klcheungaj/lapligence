// Decision S26-D16 (llg reading): the strings of one $sreadmemb/$sreadmemh call
// are consumed in order as one memory image. Words and `@` addresses continue
// from one string to the next, and the end of a string ends the number it
// holds: "1", "2" are two words, not the number 12.
//
// IEEE 1800-2009 D.14 (SystemVerilog-1800-2009.txt L70157-70160): "The
// $sreadmemh and $sreadmemb system tasks take memory data values and addresses
// as string literal arguments. The start and finish addresses indicate the
// bounds for where the data from strings will be stored in the memory. These
// strings take the same format as the strings that appear in the input files
// passed as arguments to $readmemb and $readmemh."
// 21.4 (L37276): "White space and/or comments shall be used to separate the
// numbers." The text does not say whether several strings are one image or
// separate ones, or whether a number can continue across them; llg treats the
// end of a string like white space.
module tb;
  logic [7:0] m [0:7];
  initial begin
    m = '{default: 8'h5a};
    $sreadmemh(m, 0, 3, "1", "2", "3 4");
    $display("a %h %h %h %h %h", m[0], m[1], m[2], m[3], m[4]);
    m = '{default: 8'h5a};
    $sreadmemh(m, 1, 5, "@4 aa", "bb", "@1 cc", "dd");
    $display("b %h %h %h %h %h %h", m[0], m[1], m[2], m[3], m[4], m[5]);
    $finish;
  end
endmodule
