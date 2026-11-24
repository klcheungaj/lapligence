`timescale 1ns/1ps
module tb;
  reg [7:0] v;
  initial begin : b
    parameter P = 4; parameter [7:0] Q = 8'haa;
    localparam L = P + 1; integer i; reg [P-1:0] r;
    r = ~0; v = Q; for (i = 0; i < L; i = i + 1) v = v + 1;
    $display("%0d %h %h", L, v, r);
  end
  task t; parameter X = 2; begin : tt localparam Y = X + 1; $display("%0d", Y); end endtask
  initial #1 t;
endmodule
