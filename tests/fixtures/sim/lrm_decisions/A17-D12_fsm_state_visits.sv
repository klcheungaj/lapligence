// Decision A17-D12: FSM state coverage counts, per state, the changes of
// the state expression into that state's value. Collection starts with the
// value the expression holds once the variable initializers ran: a state
// there is reached at time 0. Every later change counts, also a value the
// expression holds only in the middle of a time step. A value with unknown
// bits, or that no state names, leaves the FSM in no state. SV_COV_STOP
// stops counting; SV_COV_START counts the state the expression holds at
// that moment; SV_COV_RESET zeroes the counts and leaves the current state
// uncounted until the expression changes. A change from one state directly
// into another also counts that transition (kept in the coverage database);
// entering a state from no state, or at SV_COV_START, counts none.
// $coverage_get returns the states reached, $coverage_get_max the states.
// Here: S1 is reached by the initializer (1); S2 and S3 in one time step
// (3); X and then S3 again change nothing (3); while stopped S0 is not
// counted, and START counts it (4); after RESET nothing is reached (0)
// until the expression changes to S1 (1).
//
// IEEE 1800-2009 40.2.2 (SystemVerilog-1800-2009.txt L66038): "finite state
// machine (FSM) coverage: The number of states in an FSM that this
// simulation reached."; 40.2.1 L66023-L66025: "Finite state machine (FSM)
// coverage i) FSM states ii) FSM transitions"; 40.3.2.1 L66123-L66128:
// "`SV_COV_START, if possible, starts collecting coverage information in
// the specified hierarchy. There is no effect if coverage is already being
// collected. Coverage is automatically started at the beginning of
// simulation ..."; "`SV_COV_STOP stops collecting coverage information";
// "`SV_COV_RESET resets all available coverage information". The text does
// not say when a state counts as reached; llg counts every value the state
// expression takes, as a monitor of its writes would see them.
`timescale 1ns/1ns
module fsm;
  // tool state_vector s enum E
  logic [1:0] s = 2'd1;
  parameter [1:0] /* tool enum E */ S0 = 0, S1 = 1, S2 = 2, S3 = 3;
endmodule

module tb;
  fsm f();
  function automatic int reached();
    return $coverage_get(`SV_COV_FSM_STATE, `SV_COV_MODULE, f);
  endfunction
  initial begin
    #1 $display("initial: %0d of %0d", reached(),
                $coverage_get_max(`SV_COV_FSM_STATE, `SV_COV_MODULE, f));
    f.s = 2'd2;
    f.s = 2'd3;
    #1 $display("one step: %0d", reached());
    f.s = 2'bx;
    #1 f.s = 2'd3;
    #1 $display("unknown: %0d", reached());
    $display("stop %0d", $coverage_control(`SV_COV_STOP, `SV_COV_FSM_STATE, `SV_COV_MODULE, f));
    f.s = 2'd0;
    #1 $display("stopped: %0d", reached());
    $display("start %0d", $coverage_control(`SV_COV_START, `SV_COV_FSM_STATE, `SV_COV_MODULE, f));
    $display("started: %0d", reached());
    $display("reset %0d", $coverage_control(`SV_COV_RESET, `SV_COV_FSM_STATE, `SV_COV_MODULE, f));
    #1 $display("after reset: %0d", reached());
    f.s = 2'd1;
    #1 $display("changed: %0d", reached());
  end
endmodule
