`timescale 1ns/1ps
`include "value.vh"
module gate_cell(output [7:0] value);
  assign value = `VALUE;
endmodule
