// Decision S38-D10: a cover property statement counts a successful attempt
// as a hit, and runs its pass statement, unless the success is the vacuous
// success of an implication whose antecedent has no match, or of an if
// without else whose condition is false, passed on unchanged through not,
// nexttime, an abort that did not fire, an if without else, an implication
// consequent or a property instance. Every other success is a hit, also one
// that 16.15.8 calls vacuous: an and/or/iff/implies or until form, an
// if-else, an eventually or always form, a followed-by, and an
// accept_on abort.
//
// IEEE 1800-2009 16.15.3 (SystemVerilog-1800-2009.txt L26600-26602): the
// results of a cover property statement contain the "Number of times
// succeeded (maximum of one per attempt)" and the "Number of times
// succeeded because of vacuity"; L26604-26605: "The pass statement
// specified in statement_or_null shall be executed once for each successful
// evaluation attempt of the underlying property_spec." The text does not
// exclude vacuous successes from the pass statement; llg follows the
// industry-standard behaviour, which excludes only the vacuous successes
// of implications and conditions listed above.
//
// a, b and k stay 0, c, g and h stay 1. The clock ticks at 5, 15 and 25,
// so each cover makes three attempts; the counts are printed at 30.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0, c = 1'b1, g = 1'b1, h = 1'b1, k = 1'b0;
  property p_imp;
    a |-> b;
  endproperty
  int n_imp, n_if, n_nt, n_inst, n_nest, n_and, n_or, n_ifelse, n_until, n_ev, n_fb, n_acc;
  c_imp: cover property (@(posedge clk) a |-> b) n_imp++;
  c_if: cover property (@(posedge clk) if (a) b) n_if++;
  c_nt: cover property (@(posedge clk) nexttime (a |-> b)) n_nt++;
  c_inst: cover property (@(posedge clk) p_imp) n_inst++;
  c_nest: cover property (@(posedge clk) c |-> (a |-> b)) n_nest++;
  c_and: cover property (@(posedge clk) (a |-> b) and (a |-> b)) n_and++;
  c_or: cover property (@(posedge clk) (if (a) b) or (s_eventually k)) n_or++;
  c_ifelse: cover property (@(posedge clk) if (c) (a |-> b) else b) n_ifelse++;
  c_until: cover property (@(posedge clk) (if (a) b) until_with c) n_until++;
  c_ev: cover property (@(posedge clk) s_eventually (a |-> b)) n_ev++;
  c_fb: cover property (@(posedge clk) c #-# (a |-> b)) n_fb++;
  c_acc: cover property (@(posedge clk) accept_on (g) always h) n_acc++;
  always #5 clk = ~clk;
  initial begin
    #30;
    $display("implication %0d, if %0d, nexttime %0d, instance %0d, nested %0d",
             n_imp, n_if, n_nt, n_inst, n_nest);
    $display("and %0d, or %0d, if-else %0d, until_with %0d, s_eventually %0d",
             n_and, n_or, n_ifelse, n_until, n_ev);
    $display("followed-by %0d, accept_on %0d", n_fb, n_acc);
    $finish;
  end
endmodule
