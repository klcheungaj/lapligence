// ADV-012 diagnostics: files that cannot be read or are malformed annotate
// nothing; invalid call arguments and configuration files skip the call;
// unmatched objects, missing triple members, out-of-range values and
// oversized delay lists are reported and skipped while the rest of the file
// applies (IEEE 1800-2009 32.2, 32.9). Oracles in readme.md.
`timescale 1ns/1ps

module BUF (input a, output y);
  buf b0 (y, a);
  specify
    (a => y) = (1, 1);
  endspecify
endmodule

module FF (input clk, input d);
  reg n = 1'b0;
  specify
    $setup(d, posedge clk, 1, n);
  endspecify
endmodule

module tb;
  reg a = 1'b0, clk = 1'b0, d = 1'b0;
  wire [4:0] y;
  BUF u0 (a, y[0]);
  BUF u1 (a, y[1]);
  BUF u2 (a, y[2]);
  BUF u3 (a, y[3]);
  BUF u4 (a, y[4]);
  FF f0 (clk, d);
  initial begin
    $sdf_annotate("missing.sdf");
    $sdf_annotate("malformed.sdf");
    $sdf_annotate("unmatched.sdf");
    $sdf_annotate("values.sdf");
    $sdf_annotate("override.sdf", , , , "NOMINAL");
    $sdf_annotate("override.sdf", , , , , "1:x:2");
    $sdf_annotate("override.sdf", , , , , , "FROM_ELSEWHERE");
    $sdf_annotate("override.sdf", , "unknown.cfg");
    $monitor("%0t a=%b y=%b n=%b", $realtime, a, y, f0.n);
    #20 a = 1'b1;
    #20 a = 1'b0;
    // d changes 3 before the clock: a violation of the annotated setup 4.
    #17 d = 1'b1;
    #3 clk = 1'b1;
    #20 $finish;
  end
endmodule
