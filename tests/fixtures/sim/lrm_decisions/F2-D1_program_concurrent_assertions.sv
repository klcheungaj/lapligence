// Decision F2-D1: a concurrent assertion declared as a program item runs as
// in a module. Its attempts start at every leading clock event, it samples
// in the Preponed region, it evaluates in the Observed region, and its
// action runs in the Reactive region. This holds also when the clock is a
// program variable.
//
// IEEE 1800-2009 A.1.7 (SystemVerilog-1800-2009.txt L43147-43152):
// "non_port_program_item ::= ... | { attribute_instance }
// concurrent_assertion_item". 24.3.1 (L43270-43275): "Concurrent assertions
// are allowed in program blocks. Concurrent assertions have invariant
// scheduling semantics-whether present in program code or design code.
// Assertions always sample the values available while processing the
// Preponed region and they are always evaluated when processing the
// Observed region. If an assertion is clocked by activity on a program
// object (not recommended), the scheduler will iterate from the reactive
// region set back around the outer loop in Figure 4-1, through the Observed
// region, where the assertion is evaluated." 4.4.2.5 (L3168-3169): "During
// property evaluation, pass/fail code shall be scheduled in the Reactive
// region of the current time slot." 16.15.5 (L26681-26684): "Such a
// concurrent assertion statement uses the always semantics".
`timescale 1ns / 1ns
// The clock is a 2-state variable without an initializer and the program
// port has the same type, so the clock starts at 0 with no time-0 edge (see
// T0-D1 and T0-D2).
program prg(input bit clk);
  logic a = 1'b0;
  logic pclk = 1'b0;
  // a becomes 1 at 5 in the Re-NBA region: the tick at 5 samples 0.
  pa: cover property (@(posedge clk) a) $display("%0d pa a=%b", $time, a);
  assert property (@(negedge clk) !a) else begin : fact
    int k;
    k = 7;
    $display("%0d fail k=%0d", $time, k);
  end
  pc: cover property (@(posedge pclk) a) $display("%0d pc a=%b", $time, a);
  initial begin
    @(posedge clk) a <= 1'b1;
    #7 pclk <= 1'b1;
    #21 $display("%0d prog done", $time);
    $finish;
  end
endprogram
module tb;
  bit clk;
  always #5 clk = ~clk;
  prg p0(clk);
endmodule
