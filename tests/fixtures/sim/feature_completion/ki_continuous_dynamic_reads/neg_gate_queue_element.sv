// A gate terminal reads an element of a queue (6.21).
`timescale 1ns/1ps
module tb;
  logic pq [$] = '{1'b1, 1'b0};
  wire o;
  and g (o, pq[0], 1'b1);
  initial #1 $display("o=%b", o);
endmodule
