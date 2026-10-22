// Decision S26-D22: a real field too large for a double reads as the largest
// finite double of its sign.
//
// IEEE 1800-2009 Table 21-8 (SystemVerilog-1800-2009.txt L36922-36925):
//   "f, e, or g  Matches a floating point number. The format of a floating
//   point number is an optional sign (either + or -), followed by a string of
//   digits ... followed by an optional exponent part ..."
//
// The table gives the syntax only: it does not say what a number beyond the
// range of real (a double) reads as. llg follows industry-standard
// behaviour: %f, %e, %g and %t store +/-1.7976931348623157e308 instead of an
// infinity, also into an integral destination, which then receives that
// value converted as a real assignment would. A number too small for a
// double reads as 0.
module tb;
  real r;
  integer c;
  logic [15:0] v;
  initial begin
    c = $sscanf("1e999", "%f", r);
    $display("f c=%0d r=%e max=%0d", c, r, r == 1.7976931348623157e308);
    c = $sscanf("-1e999", "%g", r);
    $display("g c=%0d r=%e min=%0d", c, r, r == -1.7976931348623157e308);
    c = $sscanf("1e-999", "%e", r);
    $display("e c=%0d r=%e", c, r);
    c = $sscanf("2e400", "%t", r);
    $display("t c=%0d r=%e", c, r);
    v = 1;
    c = $sscanf("1e999", "%f", v);
    $display("vector c=%0d v=%h", c, v);
    $finish;
  end
endmodule
