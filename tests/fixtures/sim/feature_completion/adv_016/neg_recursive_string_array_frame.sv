// The implicit variable of a production repeated in a recursive rule is an
// unpacked array (IEEE 1800-2009 18.17.7); llg cannot save a string array.
module tb;
  initial randsequence (top)
    top : words(3) { $display("%s", words); } ;
    string words (int n) : { if (n == 0) return "."; } w w words(n - 1)
      { return {w[1], w[2], words}; } ;
    string w : { return "ab"; } ;
  endsequence
endmodule
