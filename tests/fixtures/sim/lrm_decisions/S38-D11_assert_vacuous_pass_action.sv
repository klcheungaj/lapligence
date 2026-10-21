// Decision S38-D11: an assert or assume property statement runs its pass
// statement on every success, vacuous ones included, unless assertion action
// control turns that off. An implication whose antecedent has no match, an
// if without else whose condition is false, and those forms passed through
// nexttime or another implication all succeed vacuously and run the pass
// statement.
//
// IEEE 1800-2009 16.15.1 (SystemVerilog-1800-2009.txt L26430-26431): "When
// the property for the assert statement is evaluated to be true, the pass
// statements of the action block are executed." 16.15.2 (L26477-26479):
// "When the property for the assume statement is evaluated to be true, the
// pass statements of the action_block are executed." 20.12 (L35137-35140):
// "$assertvacuousoff shall stop execution of the pass action of all the
// specified assertions on vacuous success until a subsequent $assertpasson.
// ... By default, the pass action is executed on vacuous success."
//
// a and b stay 0 and c stays 1. The clock ticks at 5, 15 and 25; every
// attempt succeeds, the nonvacuous `c` too, and the counts are printed at
// 30. A nexttime attempt resolves at the following tick, so the one started
// at 25 is still pending.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0, c = 1'b1;
  int n_imp, n_nimp, n_if, n_nt, n_nest, n_asm, n_plain, n_fail;
  a_imp: assert property (@(posedge clk) a |-> b) n_imp++; else n_fail++;
  a_nimp: assert property (@(posedge clk) a |=> b) n_nimp++; else n_fail++;
  a_if: assert property (@(posedge clk) if (a) b) n_if++; else n_fail++;
  a_nt: assert property (@(posedge clk) nexttime (a |-> b)) n_nt++; else n_fail++;
  a_nest: assert property (@(posedge clk) c |-> (a |-> b)) n_nest++; else n_fail++;
  m_asm: assume property (@(posedge clk) a |-> b) n_asm++; else n_fail++;
  a_plain: assert property (@(posedge clk) c) n_plain++; else n_fail++;
  always #5 clk = ~clk;
  initial begin
    #30;
    $display("implication %0d, nonoverlapped %0d, if %0d, nexttime %0d, nested %0d",
             n_imp, n_nimp, n_if, n_nt, n_nest);
    $display("assume %0d, nonvacuous %0d, failures %0d", n_asm, n_plain, n_fail);
    $finish;
  end
endmodule
