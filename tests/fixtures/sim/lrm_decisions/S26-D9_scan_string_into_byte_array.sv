// Decision S26-D9: %s into an unpacked array of byte stores the characters
// right-justified, as into an integral vector of the same size.
//
// IEEE 1800-2009 21.3.4.3 (SystemVerilog-1800-2009.txt L37005-37006):
//   "The string format specifier %s (or %S) may be used to read into a
//   variable of integral, unpacked array of byte, or string data types."
//
// The text does not give the element order. llg treats the array as the
// vector with the left-most element most significant: the last character
// goes to the right-most element and unused left elements become 0; a longer
// field keeps its last characters.
module tb;
  integer c;
  byte ub[4];
  byte vb[2];
  initial begin
    c = $sscanf("abc wxyz", "%s %s", ub, vb);
    $display("c=%0d ub=%h %h %h %h vb=%s%s", c, ub[0], ub[1], ub[2], ub[3], vb[0], vb[1]);
    $finish;
  end
endmodule
