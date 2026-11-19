// llg-test-fixture: tests/fixtures/sim/lrm_decisions/AA-D7_real_sampled_illegal.sv
// Decision AA-D7: a real, shortreal or realtime operand anywhere in the
// argument of `$rose` or `$fell`, or of a global clocking sampled value
// function, is a compile error. `$past`, `$stable` and `$changed` of real
// values are served by user decision (AA-D8).
//
// IEEE 1800-2009 16.6 (SystemVerilog-1800-2009.txt L21543-21544): "There are
//   certain restrictions on the expressions that can appear in concurrent
//   assertions. The restrictions on operand types, variables, and operators
//   are specified in 16.6.1, 16.6.2, and 16.6.3."
// 16.6.1 (L21574-21576): "Operand types / The following types are not
//   allowed: — Noninteger types (shortreal, real, and realtime)".
// F.3.4.4 (L70693): "$rose(e,c) $past(b,1,1,c) !== 1 && b === 1, where b is
//   the LSB of e." A real has no least significant bit.
//
// This case uses `$rose(r)` inside a concurrent assertion, where 16.6 applies
// directly. 16.9.3 does not restate the rule for procedural calls; llg applies
// the same operand rule there, which is a tool decision and is not exercised
// here.
//
// Expected result: the design is rejected at compile time; it prints nothing
// (the .out file is empty). A simulator that runs it violates 16.6.1.
module tb;
  logic clk = 1'b0;
  real r = 0.0;
  always #5 clk = ~clk;
  always @(posedge clk) r <= r + 0.5;
  a_rose: assert property (@(posedge clk) $rose(r))
    else $display("a_rose fail t=%0d", $time);
  initial #30 $finish;
endmodule
