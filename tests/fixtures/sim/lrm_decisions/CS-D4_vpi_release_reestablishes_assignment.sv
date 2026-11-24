// Decision CS-D4: vpi_put_value with vpiReleaseFlag on a variable that a
// continuous assignment or an output port drives re-establishes the
// assignment like a release statement (CS-D3): the variable holds the driven
// value when vpi_put_value returns, also when the assignment's operands do
// not change afterwards. A variable only procedures write keeps the forced
// value until its next procedural assignment.
//
// IEEE 1800-2009 38.34 (SystemVerilog-1800-2009.txt L64189-64190):
// "vpiReleaseFlag The object shall be released from a forced value (same as
// the SystemVerilog procedural release)."
// 10.6.2 L13390-13395: "if the variable is not driven by a continuous
// assignment ... the variable shall not immediately change value and shall
// maintain its current value until the next procedural assignment to the
// variable is executed. Releasing a variable that is driven by a continuous
// assignment ... shall reestablish that assignment".
module stage (
    output logic [7:0] q,
    input  logic [7:0] d
);
  assign q = d + 8'd1;
endmodule

module tb;
  logic [7:0] in, ported, assigned, stored;
  stage u (
      .q(ported),
      .d(in)
  );
  assign assigned = in + 8'd2;
  always @(in) stored = in + 8'd3;
  initial begin
    in = 8'd1;
    #1 $d4(1);
    #1 $display("forced: ported=%0d assigned=%0d stored=%0d", ported, assigned, stored);
    $d4(0);
    $display("released: ported=%0d assigned=%0d stored=%0d", ported, assigned, stored);
    in = 8'd5;
    #1 $display("next operand: ported=%0d assigned=%0d stored=%0d", ported, assigned, stored);
    $finish;
  end
endmodule
