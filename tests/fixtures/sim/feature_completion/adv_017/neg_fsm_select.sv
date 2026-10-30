// A part-select outside the declared range of the state signal (IEEE
// 1800-2009 40.4.2).
module tb;
  logic [7:0] r = 0;
  /* tool state_vector r[9:8] field enum E */
  parameter [1:0] /* tool enum E */ S0 = 0, S1 = 1;
  initial $display("%0d", $coverage_get(`SV_COV_FSM_STATE, `SV_COV_HIER, tb));
endmodule
