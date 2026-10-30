// A state vector without a parameter declaration that lists its states
// (IEEE 1800-2009 40.4.6): llg does not extract FSM states automatically.
module tb;
  // tool state_vector cs enum E
  logic [1:0] cs = 0;
  initial $display("%0d", cs);
endmodule
