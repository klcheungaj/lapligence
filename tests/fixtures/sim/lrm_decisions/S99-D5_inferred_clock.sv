// Decision S99-D5: a procedural concurrent assertion without a clock takes
// the clock inferred from its procedure, before default clocking; an `iff`
// qualifier stays part of the inferred clock; a procedure that reads a term
// of its edge expression anywhere else in its body (including assertion
// action blocks) infers no clock, and default clocking applies. Nonblocking
// intra-assignment event controls count as event controls of the procedure.
//
// IEEE 1800-2009 16.15.6 (SystemVerilog-1800-2009.txt L26782-26798): "If no
// clocking event is specified in a procedural concurrent assertion, the
// leading clocking event of the assertion shall be inferred from the
// procedural context, if possible. If no clock can be inferred from the
// procedural context, then the clocks shall be inferred from the default
// clocking (14.12)" ... "c) Within the event control of the procedure, there
// is exactly one event expression that satisfies both of the following
// conditions: 1) The event expression is of the form edge_identifier
// expression1 [ iff expression2 ] and is not a proper subexpression of an
// event expression of this form. 2) No term in expression1 appears anywhere
// else in the body of the procedure." The text does not say whether action
// blocks or intra-assignment event controls are part of the procedure; llg
// reading: they are.
// i1 is clocked by posedge clk (5, 15, 25), i3 by the default clocking
// posedge clk2 (12, 20, 28) because its procedure reads clk, and i2 by
// negedge clk iff en (30 only).
`timescale 1ns/1ns
module tb;
  logic clk = 0, clk2 = 0;
  logic a = 0, en = 0, q1, q2;
  always #5 clk = ~clk;
  always #4 clk2 = ~clk2;
  default clocking @(posedge clk2); endclocking
  always @(posedge clk) begin
    q1 <= a;
    i1: assert property (a) $display("%0t i1 pass", $time); else $display("%0t i1 fail", $time);
  end
  always @(posedge clk) begin
    q2 <= clk;
    i3: assert property (a) $display("%0t i3 pass", $time); else $display("%0t i3 fail", $time);
  end
  always @(negedge clk iff en) begin
    i2: assert property (a) $display("%0t i2 pass", $time); else $display("%0t i2 fail", $time);
  end
  initial begin
    #12 a = 1;
    #10 en = 1;
    #9 $finish;
  end
endmodule
