// A state_vector pragma that names no signal of its module (IEEE 1800-2009
// 40.4.1): an error, not a silently uncovered FSM.
module tb;
  logic [1:0] cs = 0;
  /* tool state_vector cz enum E */
  parameter [1:0] /* tool enum E */ S0 = 0, S1 = 1;
  initial $display("%0d", cs);
endmodule
