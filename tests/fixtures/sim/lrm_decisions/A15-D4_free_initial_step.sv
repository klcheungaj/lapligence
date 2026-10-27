// Decision A15-D4: an initialized active free checker variable keeps its
// initial value in the first time step, and an active free variable is read
// with its current value. k is declared rand with initializer 9 and is never
// assigned, so it is active and may be redrawn at later time steps; the
// deferred assertion of the checker initial procedure runs at time 0 and
// reads k = 9. z is redrawn before the Observed region of every time step
// with its clock event, and the assume constrains it to 2.
//
// IEEE 1800-2009 17.7 (SystemVerilog-1800-2009.txt L28766, L28777-28780): "If a
// non-constant free variable has been initialized but is never assigned then
// it can ... be randomized in subsequent time steps in simulation (see
// 17.7.2), except the first one where its value is defined by the
// initialization." 17.7.2 L28976-28979: "When an implementation is about to
// begin the Observed region, it shall solve for all the active free
// variables. ... Checker procedures and properties shall not use sampled
// values of active free checker variables; current values shall be read so
// that up-to-date solved values are visible."
`timescale 1ns / 1ns
checker c(event clk);
  rand bit [3:0] k = 4'd9;
  rand bit [3:0] z;
  m1: assume property (@clk z == 4'd2);
  initial a_k: assert #0 (k == 4'd9) $display("%0t k=%0d", $time, k);
  c1: cover property (@clk z == 4'd2) $display("%0t z=%0d", $time, z);
endchecker
module tb;
  logic clk = 1'b0;
  c u(posedge clk);
  always #5 clk = ~clk;
  initial #22 $finish;
endmodule
