// Decision A19-D45: statement and toggle coverage through the VPI. Without
// a $coverage_* call naming them, a model built for VPI applications has
// statement and toggle coverage available but not collecting until
// vpi_control(vpiCoverageStart, ...) for an instance. vpi_get(
// vpiStatementCoverage | vpiToggleCoverage, instance) is the number of
// covered statements or bits that the instance itself declares (not its
// child instances). On a statement handle vpiCovered, vpiCoverMax and
// vpiCoveredCount count the statement and every statement it contains
// (covered ones, all, and the sum of their executions); on a net,
// variable or bit handle they count its bits (seen at both 0 and 1, all,
// and the sum of their changes between 0 and 1); a variable without
// toggle coverage (a real, an array) has no coverable bits. vpi_control
// with a statement or signal handle fails with SV_COV_ERROR and a VPI
// error.
//
// IEEE 1800-2009 40.5.3 (SystemVerilog-1800-2009.txt L66585-66590):
// "vpi_get(<coverageType>, instance_handle) returns the number of covered
// items of the given coverage type in the given instance." L66598-66603:
// "For handles that can contain multiple coverable entities, such as
// statement, FSM, and signal handles, the return value indicates how many
// of the entities have been covered. ... For statement handle, the
// entities are statements. For signal handle, the entities are individual
// signal bits." L66722-66725: "When the handle points to an item
// containing multiple coverable entities (such as a handle to a block
// statement containing a number of statements), the result is the sum of
// coverage counts for each of the constituent entities." L66733-66734:
// "returns the number of coverable entities pointed by the handle."
// 40.5.4 L66769-66770: "Statement, toggle, and FSM coverage are not
// individually controllable (i.e., they are controllable only at the
// instance level and not on a per-statement, signal, or FSM basis)." What
// the count of a toggle bit is, and whether an instance's number includes
// its children, are not stated.
module leaf(input logic go);
  logic [1:0] n = 2'b00;
  real r = 0.0;
  always @(posedge go) begin
    n++;
    if (n > 2'd1) n = 2'd0;
  end
endmodule

module tb;
  logic go = 1'b0;
  leaf u(go);
  initial begin
    // After every process has reached its first wait (4.4.2.3).
    #0 $d45(0);
    repeat (3) begin
      #1 go = 1'b1;
      #1 go = 1'b0;
    end
    // After the last port update has reached tb.u.go.
    #1 $d45(1);
    $finish;
  end
endmodule
