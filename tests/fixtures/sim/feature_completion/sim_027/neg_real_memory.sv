// Memory files hold binary or hexadecimal numbers for integral words
// (IEEE 1800-2009 21.4, 21.4.1); real words are not memory-file words.
module tb;
  real mem [0:3];
  initial $readmemh("in.hex", mem);
endmodule
