// Decision A20-D16: the delay expressions of vpiDelay. A primitive, a
// primitive array, a continuous assignment, a net, a module path, a timing
// check and a delay control give the delay expression of their source: one
// delay is that expression (a constant, an operation, a variable), several
// delays are a vpiOperation of vpiOpType vpiListOp whose operands are the
// delays in source order, and a min:typ:max delay is the operand the model's
// min:typ:max selection uses. An object declared without a delay gives NULL
// without an error. A timing check gives its limits (one limit: that
// expression; two: a vpiListOp), a net declared with a delay gives that
// delay; a port has no delay expression (a vpiError). vpi_get_delays reads
// the delays the tool uses.
//
// IEEE 1800-2009 37.3.4 (SystemVerilog-1800-2009.txt L57417-L57424):
// "Nets, primitives, module paths, timing checks, and continuous assignments
// can have delays specified within the SystemVerilog source code. [...] To
// access the delay expressions that are specified within the SystemVerilog
// source code, use the method vpiDelay. These expressions shall be either an
// expression that evaluates to a constant if there is only one delay
// specified or an operation if there are more than one delay specified. If
// multiple delays are specified, then the operation's vpiOpType shall be
// vpiListOp."; the diagrams draw vpiDelay for primitive arrays (37.11
// L58008), primitives (37.31 L59747), module paths (37.35 L59878), timing
// checks (37.36 L59928), continuous assignments (37.43 L60277) and delay
// controls (37.63 L61200). Some simulators give NULL for a timing check;
// llg follows the clause text, which lists timing checks among the objects
// with delay expressions and draws the relation. The operand a min:typ:max
// delay gives and the NULL of an undelayed object are llg choices that
// match industry-standard behaviour.
//
// Build A20-D16_delay_expressions.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d16()` declared as a system task).
`timescale 1ns/1ns
module dut (input a, input clk, output y);
  buf ib (y, a);
  specify
    (a => y) = (1, 2);
    $width(posedge clk, 4);
    $setuphold(posedge clk, a, 2, 1);
  endspecify
endmodule

module tb;
  reg a = 1'b0, clk = 1'b0;
  reg [3:0] d = 4'd2;
  wire y, w0, w1, w2, w3, w4, c1, c2, nn;
  wire [1:0] wa;
  wire #5 nd;
  dut u (.a(a), .clk(clk), .y(y));
  buf b0 (w0, a);
  buf #3 b1 (w1, a);
  bufif1 #(1, 2, 3) b2 (w2, a, a);
  buf #(1:2:3) b3 (w3, a);
  buf #(d) b4 (w4, a);
  buf #2 ga[1:0] (wa, a);
  assign #(1 + 1) c1 = a;
  assign #(1, 2) c2 = a;
  assign nd = a;
  assign nn = a;
  initial begin
    #4 $d16();
  end
endmodule
