`timescale 1ns/1ns
// ADV-008 negative: IEEE 1364-2001 has no `edge` identifier for
// edge-sensitive paths (14.2.4: edge_identifier ::= posedge | negedge); the
// keyword is SystemVerilog only (IEEE 1800-2009 30.4.3).
module ae(input clk, d, output q);
  assign q = d;
  specify
    (edge clk => (q +: d)) = 10;
  endspecify
endmodule

module tb;
  reg clk, d;
  wire q;
  ae u(.clk(clk), .d(d), .q(q));
endmodule
