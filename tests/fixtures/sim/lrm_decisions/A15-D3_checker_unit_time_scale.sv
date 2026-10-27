// Decision A15-D3: a checker declared in the compilation unit takes the time
// scale of the compilation unit, which only a timeunit declaration sets: a
// preceding `timescale directive does not apply to it, so without timeunit
// it uses the default unit 1ns (F2-D2). The module's posedge clk at 1000ps
// and 3000ps is $time 1 and 3 in the checker.
//
// IEEE 1800-2009 3.14.2.3 (SystemVerilog-1800-2009.txt L2946): "If a
// timeunit is not specified within a module, program, package, or interface
// definition" (checkers are not listed); L2956-2957: "The time unit of the
// compilation-unit scope can only be set by a timeunit declaration, not a
// `timescale directive. If it is not specified, then the default time unit
// shall be used."; 17.2 L28154-28161: "A checker may be declared in one of
// the following: ... A compilation unit scope".
`timescale 1ps / 1ps
checker ticks(event clk);
  c1: cover property (@clk 1'b1) $display("checker %0d", $time);
endchecker

module tb;
  logic clk = 1'b0;
  always #1000 clk = ~clk;
  always @(posedge clk) $display("tb %0d", $time);
  ticks t(posedge clk);
  initial #3500 $finish;
endmodule
