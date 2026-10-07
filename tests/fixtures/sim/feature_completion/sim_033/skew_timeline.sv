`timescale 1ns/1ns
// SIM-033 A01: #1step, #0, positive and default input skews on an irregular
// clock with same-slot blocking and nonblocking updates of the source.
module tb;
  parameter int SK = 3;
  logic clk = 1'b0;
  logic [7:0] d = 8'h00;

  clocking cb @(posedge clk);
    input #1step s1 = d;
    input #0 s0 = d;
    input #2 s2 = d;
    input #SK s3 = d;
    input sd = d;
    input #2 sw = {d[3:0], d[7:4]};
  endclocking

  always @(posedge clk) d <= d + 8'h01;

  always @(cb)
    $display("%0t s1=%h s0=%h s2=%h s3=%h sd=%h sw=%h d=%h",
             $time, cb.s1, cb.s0, cb.s2, cb.s3, cb.sd, cb.sw, d);

  initial begin
    #1 d = 8'h10;
    #3 clk = 1'b1;
    #1 clk = 1'b0;
    #1 clk = 1'b1;
    #1 clk = 1'b0;
    d = 8'h40;
    #4 d = 8'h50;
    #2 d = 8'h60;
    clk = 1'b1;
    #1 clk = 1'b0;
    #1 clk = 1'b1;
    #1 clk = 1'b0;
    #24 clk = 1'b1;
    #1 $finish;
  end
endmodule
