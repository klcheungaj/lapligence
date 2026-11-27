// A net declaration assignment reads an element of an associative array (6.21).
`timescale 1ns/1ps
module tb;
  logic [31:0] aa [string] = '{"k": 7};
  wire [31:0] w = aa["k"];
  initial #1 $display("w=%0d", w);
endmodule
