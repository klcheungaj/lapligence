// A continuous assignment reads a member of a record element of a queue (6.21).
`timescale 1ns/1ps
module tb;
  typedef struct {
    int a;
  } r_t;
  r_t rq [$] = '{'{a: 5}};
  int x;
  assign x = rq[0].a;
  initial #1 $display("x=%0d", x);
endmodule
