// Decision S24-D2 (singular values): a singular integral argument of %p
// prints as it would unformatted, the decimal conversion in its automatic
// field (the width of %d: the digits of the largest unsigned value of its
// width, and 11 characters for a signed value of 2 to 32 bits, S24-D1);
// %0p prints it in the minimum width. The same integral value as an element
// or member inside an aggregate prints its digits alone, with no leading
// spaces. An enum prints its name and a string its quoted text.
// A real or shortreal prints in the shortest form of C's %g, in %p and %0p,
// at top level and inside aggregates: six significant digits, an exponent
// outside 1e-4..1e6, no trailing zeros (1.5, 2, 1e+20, 1.23457e+06), and
// inf and -inf for infinities.
//
// IEEE 1800-2009 21.2.1.7 (SystemVerilog-1800-2009.txt L36415):
//   "All other singular data types shall print their values as they would
//   unformatted."
// L36425-36426: "The %p and %0p format specifiers can also be used to print
//   singular expressions, in which case the expression is formatted as an
//   element of an aggregate expression described above."
// L36402-36404: "The use of white space is implementation dependent;
//   however the output shall be a legal interpretation of the assignment
//   pattern syntax (see 10.8)."
// 21.2.1.2 (L35976-35977): "Any expression argument that has no
//   corresponding format specification is displayed using the default
//   decimal format in $display and $write"
// 21.2.1.3 (L36149): "For expression arguments, the values written to the
//   output file (or terminal) are sized automatically."
//
// The text does not say how an unformatted real is written; llg follows
// industry-standard behaviour (C's %g), which is also how that output
// prints an unformatted real argument.
module tb;
  typedef enum {ON, OFF} sw_e;
  typedef struct { int a; byte b; shortint c; } us_t;
  typedef struct { int a; string s; byte b; real r; } rec_t;
  typedef struct { real r; shortreal s; } rs_t;
  logic [3:0] l = 4'h5;
  logic [7:0] lx = 8'b1x0z_0101;
  logic [7:0] lz = 8'bzzzz_zzzz;
  logic l1 = 1'b1;
  int i = -2;
  int unsigned iu = 32'hffff_fffe;
  byte by = -1;
  byte unsigned bu = 8'd200;
  shortint si = 7;
  longint li = -64'sd5;
  longint unsigned lu = 64'd18446744073709551615;
  bit [64:0] b65 = 65'd5;
  integer igx;
  time tm = 64'd99;
  sw_e e = OFF;
  string s = "ab";
  int q[$] = '{5, -6};
  int fa[2] = '{1, 22};
  us_t us = '{3, -4, 9};
  rec_t rc = '{-5, "hi", 8'sd4, 2.5};
  real r15 = 1.5, r2 = 2.0, r01 = 0.1, r1e20 = 1e20, r1em7 = 1e-7, rnz = -0.0;
  real rbig = 123456789.0, r1m = 1234567.0, r6 = 123456.0, rsmall = 0.0001, rsm2 = 0.00001;
  real rpi = 3.14159265358979, r0 = 0.0, r333 = 1.0 / 3.0;
  real rinf, rninf;
  shortreal s01 = 0.1, s3 = 1.0 / 3.0;
  real rq[$];
  real ra[2][2] = '{'{1.0, 0.5}, '{1e-5, 123456789.0}};
  rs_t st;
  initial begin
    rinf = 1.0 / r0;
    rninf = -1.0 / r0;
    $display("l=[%p] l0=[%0p] lx=[%p] lz=[%p] l1=[%p]", l, l, lx, lz, l1);
    $display("i=[%p] i0=[%0p] iu=[%p] by=[%p] bu=[%p] si=[%p]", i, i, iu, by, bu, si);
    $display("li=[%p] lu=[%p] b65=[%p] igx=[%p] tm=[%p]", li, lu, b65, igx, tm);
    $display("e=[%p] s=[%p] q0=[%p] fa1=[%p] lit=[%p] sz=[%p] cat=[%p]",
             e, s, q[0], fa[1], 5, 4'd3, {l, l});
    $display("q=[%p] fa=[%p] us=[%p] rc=[%p]", q, fa, us, rc);
    $display("mem=[%p] memr=[%p]", rc.a, rc.r);
    $display("sf=[%s]", $sformatf("%p|%0p", by, by));
    $write("w=[%p]\n", si);
    rq = '{r15, r2, r01, r1e20, r1em7, rnz, rbig, r1m, r6, rsmall, rsm2, rpi, r0, r333, rinf, rninf};
    $display("rq=[%p]", rq);
    $display("ra=[%p]", ra);
    st = '{r01, s01};
    $display("st=[%p] sq=[%p %p]", st, s01, s3);
    $display("top=[%p] [%p] [%p] [%p] [%0p] [%0p]", r15, r2, r1e20, rinf, r1m, r01);
    $display("sfr=[%s]", $sformatf("%p", r333));
    $finish;
  end
endmodule
