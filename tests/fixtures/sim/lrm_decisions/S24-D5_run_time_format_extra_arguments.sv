// Decision S24-D5: the arguments that the format string of $sformat or
// $sformatf does not consume print in their default formats after it, whether
// the format is a literal or known only at run time (a string or an integral
// value, which reads as %s prints it). llg reports the surplus of a run-time
// format once per run as a warning.
//
// IEEE 1800-2009 21.3.3 (SystemVerilog-1800-2009.txt L36754-36757):
//   "The remaining arguments to $sformat, if any, are processed using any
//   format specifiers in the format_string, until all such format specifiers
//   are used up. If not enough arguments are supplied for the format
//   specifiers or too many are supplied, then the application shall issue a
//   warning and continue execution."
// L36749-36751: "This format argument can be a string literal, such as
//   "data is %d", or may be a an expression of integral, unpacked array of
//   byte, or string data types whose content is interpreted as the formatting
//   string."
// 21.2.1.2 L35976-35978 gives the default format. A string argument is not
// part of the case (industry-standard output prints its bytes in decimal, llg
// its text).
module tb;
  string f, so;
  reg [8*10:1] rf;
  reg [8*20:1] ro;
  int a = 5, b = 7;
  real r = 1.5;
  logic [3:0] n = 4'b1x0z;
  time t = 12;

  initial begin
    f = "x=%0d";
    $display("1:%s", $sformatf(f, a, b));
    $display("2:%s", $sformatf("x=%0d", a, b));
    $display("3:%s", $sformatf(f, a, b, r, n));
    $display("4:%s", $sformatf("x=%0d", a, b, r, n));
    $sformat(so, f, a, b);
    $display("5:%s", so);
    $sformat(ro, f, a, b);
    $display("6:%0s", ro);
    f = "none";
    $display("7:%s", $sformatf(f, a));
    f = "";
    $display("8:%s", $sformatf(f, a, b));
    f = "50%%";
    $display("9:%s", $sformatf(f, a));
    f = "%m";
    $display("10:%s", $sformatf(f, a));
    f = "x";
    $display("11:%s", $sformatf(f, t, r, 8'hff, -1));
    $display("12:%s", $sformatf("x", t, r, 8'hff, -1));
    f = "%0d %0d";
    $display("13:%s", $sformatf(f, a, b));
    rf = "r=%0d";
    $display("14:%s", $sformatf(rf, a, b));
    $sformat(so, rf, a);
    $display("15:%s", so);
    $finish;
  end
endmodule
