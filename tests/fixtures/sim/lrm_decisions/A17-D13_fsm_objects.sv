// Decision A17-D13: the FSM objects of the VPI. vpi_iterate(vpiFsm, h)
// takes a module or interface instance and lists the FSMs that instance
// itself declares (not those below it); a NULL or other handle is an
// error. An FSM handle has type vpiFsm, its states type vpiFsmHandle, in
// declaration order. vpi_get_value of a state is its parameter value, as
// wide as the state expression. On an FSM, vpiCovered is the number of
// states reached, vpiCoverMax the number of states and vpiCoveredCount the
// sum of the state visits (A17-D12); on a state, vpiCovered is 0 or 1,
// vpiCoverMax 1 and vpiCoveredCount its visits. vpi_get(vpiFsmStateCoverage,
// instance) is the number of states reached by that instance's FSMs.
// vpiFsmStateExpression is the state signal or part-select; a concatenation
// has no VPI object in llg, so the relation is NULL with an error. FSM
// coverage is controlled per instance only: vpi_control with an FSM or
// state handle is an error. Here f.s visits S1 (initializer), S2 and S0;
// the concatenation {a, b} reaches P1 once.
//
// IEEE 1800-2009 40.5.3 (SystemVerilog-1800-2009.txt L66738): "returns an
// iterator to all FSMs in an instance."; L66742 "returns the handle to the
// signal or expression encoding the FSM state."; L66746 "returns an
// iterator to all states of an FSM."; L66750 "returns the value of an FSM
// state."; L66604 "For FSM handle, the entities are FSM states.";
// L66721-L66724 vpiCoveredCount "returns the number of times each coverable
// entity referred by the handle has been covered ... the result is the sum
// of coverage counts for each of the constituent entities."; L66733 "The
// number returned shall always be 1 when applied to an assertion or FSM
// state handle."; 40.5.4 L66769-L66770: "Statement, toggle, and FSM coverage
// are not individually controllable (i.e., they are controllable only at
// the instance level and not on a per-statement, signal, or FSM basis)."
// Annex M L76725-L76726: vpiFsm 758, vpiFsmHandle 759. llg choice: the type
// of a state handle (vpiFsmHandle) and the missing concatenation object.
`timescale 1ns/1ns
module fsm;
  // tool state_vector s enum E
  logic [3:0] s = 4'd1;
  parameter [3:0] /* tool enum E */ S0 = 0, S1 = 1, S2 = 2;
  logic a = 0, b = 0;
  /* tool state_vector {a, b} pair enum P */
  parameter [1:0] /* tool enum P */ P1 = 1, P2 = 2;
endmodule

module tb;
  fsm f();
  initial begin
    #1 f.s = 4'd2;
    #1 f.s = 4'd0;
    {f.a, f.b} = 2'b01;
    #1 $d13;
  end
endmodule
