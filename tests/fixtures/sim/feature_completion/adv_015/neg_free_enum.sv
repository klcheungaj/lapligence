// FIN-001 A3 limit: an enum free checker variable (IEEE 1800-2009 17.7,
// 17.7.2) is a located limit; llg randomizes packed integral free variables.
`timescale 1ns / 1ns
typedef enum logic [1:0] {A, B, C} e_t;
checker bad(event clk);
  rand e_t f;
  m1: assume property (@clk f inside {A, B});
endchecker
module tb;
  logic clk = 1'b0;
  bad u(posedge clk);
  initial #10 $finish;
endmodule
