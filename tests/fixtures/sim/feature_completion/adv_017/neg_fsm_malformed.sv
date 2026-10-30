// A state_vector pragma whose enum keyword has no enumeration name.
module tb;
  logic [1:0] cs = 0;
  /* tool state_vector cs enum */
  parameter [1:0] /* tool enum E */ S0 = 0, S1 = 1;
  initial $display("%0d", cs);
endmodule
