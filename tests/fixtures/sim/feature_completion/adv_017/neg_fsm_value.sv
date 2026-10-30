// A state whose parameter value does not fit the 2-bit state vector
// (IEEE 1800-2009 40.4.6).
module tb;
  logic [1:0] cs = 0;
  // tool state_vector cs enum E
  parameter /* tool enum E */ S0 = 0, S4 = 4;
  initial $display("%0d", $coverage_get(`SV_COV_FSM_STATE, `SV_COV_HIER, tb));
endmodule
