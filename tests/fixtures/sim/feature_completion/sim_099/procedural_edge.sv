`timescale 1ns/1ns
// 16.15.6 c): `edge clk` is the clock the procedure infers, the clocking
// event of both edges of clk; it ticks at 5, 10, ... (a `bit` keeps time 0
// free of an initializer edge).
module tb;
  bit clk;
  logic a = 1;
  always #5 clk = ~clk;
  always @(edge clk) begin
    r1: assert property (a) $display("[%0t] r1 pass", $time);
      else $display("[%0t] r1 fail", $time);
    r2: assert property (1 ##1 a) $display("[%0t] r2 pass", $time);
      else $display("[%0t] r2 fail", $time);
  end
  initial begin
    #12 a = 1'b0;
    #10 a = 1'b1;
  end
  initial #31 $finish;
endmodule
