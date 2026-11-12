// A state vector wider than 64 bits: llg covers FSM state vectors of 1 to
// 64 bits (llg limitation, A17-D11).
module tb;
  logic [64:0] cs = 0;
  // tool state_vector cs enum E
  parameter [64:0] /* tool enum E */ S0 = 0, S1 = 1;
  initial $display("%0d", $coverage_get(`SV_COV_FSM_STATE, `SV_COV_HIER, tb));
endmodule
