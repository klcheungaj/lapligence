`timescale 1ns/1ps
`include "value.vh"
module rtl_cell(output [7:0] value);
  assign value = `VALUE;
endmodule
