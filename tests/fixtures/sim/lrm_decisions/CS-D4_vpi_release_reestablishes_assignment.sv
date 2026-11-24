// Decision CS-D4: vpi_put_value with vpiReleaseFlag on a variable that a
// continuous assignment or an output port drives re-establishes the
// assignment like a release statement, also when the assignment's operands
// do not change afterwards. A driver that is a copy of another variable (a
// port connection), a constant or a delayed assignment gives its value
// before vpi_put_value returns (CS-D3, CS-D5). An assignment of an
// expression is scheduled: the variable holds the driven value when the
// time step ends, and until the assignment has run a read still sees the
// forced value. A variable only procedures write keeps the forced value
// until its next procedural assignment.
//
// IEEE 1800-2009 38.34 (SystemVerilog-1800-2009.txt L64189-64190):
// "vpiReleaseFlag The object shall be released from a forced value (same as
// the SystemVerilog procedural release)."
// 10.6.2 L13390-13395: "if the variable is not driven by a continuous
// assignment ... the variable shall not immediately change value and shall
// maintain its current value until the next procedural assignment to the
// variable is executed. Releasing a variable that is driven by a continuous
// assignment ... shall reestablish that assignment and schedule a
// reevaluation in the continuous assignment's scheduling region."
// The case reads the expression-driven variable in the Postponed region
// only ($strobe), where every scheduled reevaluation has run; some
// simulators deliver it before vpi_put_value returns, which 4.7 also allows.
`timescale 1ns / 1ns
module stage (
    output logic [7:0] q,
    input  logic [7:0] d
);
  assign q = d + 8'd1;
endmodule

module tb;
  logic [7:0] in, ported, assigned, stored, tied, delayed;
  stage u (
      .q(ported),
      .d(in)
  );
  assign assigned = in + 8'd2;
  assign tied = 8'd77;
  assign #3 delayed = in + 8'd4;
  always @(in) stored = in + 8'd3;

  task show(input string label);
    $display("%s: ported=%0d tied=%0d delayed=%0d stored=%0d", label, ported, tied, delayed,
             stored);
  endtask

  initial begin
    in = 8'd1;
    #5 $d4(1);
    #1 show("forced");
    $display("forced: assigned=%0d", assigned);
    // The operand changes while the forces hold and not again before the
    // release.
    in = 8'd7;
    #4 $d4(0);
    show("at the release");
    $strobe("end of the time step: assigned=%0d", assigned);
    #1 show("one unit later");
    $display("one unit later: assigned=%0d", assigned);
    in = 8'd20;
    #4 show("next operand");
    $display("next operand: assigned=%0d", assigned);
    $finish;
  end
endmodule
