// Decision S26-D6: an integer conversion into a real destination assigns
// the integer value converted to real.
//
// IEEE 1800-2009 21.3.4.3 (SystemVerilog-1800-2009.txt L37001-37002):
//   "The integer format specifiers, %h (or %H), %d (or %D), %o (or %O), %b
//   (or %B), %c (or %C), %u (or %U), and %z (or %Z), may be used to read into
//   any of the integral data types"
//
// The text names integral destinations only. llg also accepts real ones
// and converts the integer as an assignment would (6.12.2); a value with X or
// Z bits converts as 0, as in assignment.
module tb;
  integer c;
  real r1, r2, r3;
  initial begin
    c = $sscanf("7 ff 101", "%d %h %b", r1, r2, r3);
    $display("c=%0d r1=%0.1f r2=%0.1f r3=%0.1f", c, r1, r2, r3);
    $finish;
  end
endmodule
