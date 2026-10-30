// ADV-019 runtime structures: user systf iteration, callback iteration,
// the time queue, vpiUse and UDP definitions (IEEE 1800-2009 37.32, 37.38,
// 37.54, 37.75, 37.76, 37.79). Expected output: readme.md.
`timescale 1ns / 1ns
primitive rt_or (out, a, b);
  output out;
  input a, b;
  table
    1 ? : 1;
    ? 1 : 1;
    0 0 : 0;
  endtable
endprimitive

primitive rt_latch (q, d, en);
  output q;
  reg q;
  input d, en;
  initial q = 1'bx;
  table
    ? 0 : ? : -;
    0 1 : ? : 0;
    1 1 : ? : 1;
  endtable
endprimitive

module tb;
  reg [1:0] r = 0;
  reg d = 0, en = 0;
  wire o, q;
  rt_or u1 (o, r[0], r[1]);
  rt_latch u2 (q, d, en);
  wire w;
  assign w = r[1];
  initial begin
    #1 $runtime(1);
    r = 1;
    #2 $runtime(2);
    #10 $finish;
  end
  initial #5 en = 1;
endmodule
