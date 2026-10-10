// SIM-026 A01: every $sscanf conversion of IEEE 1800-2009 Table 21-8 with
// field widths, X/Z digits, signed input, suppression, literal matching,
// overlong values (least significant bits kept, +Inf for reals), %t scaled by
// $timeformat into the scope's time unit, %v strengths, %m, and sources and
// formats held in packed vectors, strings and byte arrays.
`timescale 1ns / 100ps
module tb;
  integer c;
  logic [69:0] w70;
  logic [3:0] n4;
  logic [7:0] b8, c8, o8, v8;
  logic signed [7:0] s8;
  logic [15:0] h16;
  logic [31:0] i32;
  real r1, r2, r3, r4, r5;
  realtime t;
  time ti;
  string s, m, p, fmt;
  logic [8*5-1:0] packed_src;
  logic [8*8-1:0] padded_src;
  byte bytes_src[4];
  initial begin
    c = $sscanf("539762694060454855883 10x1 7z -17 FfZ", "%d %b %o %d %h", w70, n4, o8, s8, h16);
    $display("A c=%0d w70=%h n4=%b o8=%b s8=%0d h16=%h", c, w70, n4, o8, s8, h16);
    c = $sscanf("A BCDEF gh 12%34", "%c %3s%s %s %d%%%d", c8, s, m, p, b8, i32);
    $display("B c=%0d c8=%h s=%s m=%s p=%s b8=%0d i32=%0d", c, c8, s, m, p, b8, i32);
    c = $sscanf(" A", "%c", c8);
    $display("C c=%0d c8=%h", c, c8);
    c = $sscanf("2.5e1 -0.125 7 1e400 ff", "%e %g %d %f %h", r1, r2, r3, r4, r5);
    $display("D c=%0d r1=%f r2=%f r3=%f r4=%f r5=%f", c, r1, r2, r3, r4, r5);
    c = $sscanf("300 1ff 5 6 7", "%d %h %*d %d %*s", b8, c8, s8);
    $display("E c=%0d b8=%0d c8=%h s8=%0d", c, b8, c8, s8);
    $timeformat(-3, 2, " ms", 10);
    c = $sscanf("10.345 2", "%t %t", t, ti);
    $display("F c=%0d t=%f ti=%0d", c, t, ti);
    c = $sscanf("St1 HiZ We0", "%v %v %v", v8[0], v8[1], v8[2]);
    $display("G c=%0d v=%b", c, v8[2:0]);
    c = $sscanf("12345", "%m%3d", s, i32);
    $display("H c=%0d s=%s i32=%0d", c, s, i32);
    packed_src = "12 34";
    padded_src = "7 8";
    bytes_src = '{"5", " ", "6", 0};
    fmt = "%d %d";
    c = $sscanf(packed_src, fmt, b8, c8);
    $display("I c=%0d b8=%0d c8=%0d", c, b8, c8);
    c = $sscanf(padded_src, "%d %d", b8, c8);
    $display("J c=%0d b8=%0d c8=%0d", c, b8, c8);
    c = $sscanf(bytes_src, "%d %d", b8, c8);
    $display("K c=%0d b8=%0d c8=%0d", c, b8, c8);
    $finish;
  end
endmodule
