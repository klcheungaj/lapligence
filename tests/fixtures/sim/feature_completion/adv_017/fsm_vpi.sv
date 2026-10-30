// ADV-017 FSM objects of the VPI (IEEE 1800-2009 40.5.2-40.5.4): vpiFsm,
// vpiFsmStates, vpiFsmStateExpression, the coverage properties of FSMs and
// states, vpi_control of FSM coverage and a value-change callback on a
// state vector. Companion of fsm_vpi.c. Derivations: readme.md.
`timescale 1ns/1ns
module ctrl(input logic clk, input logic go);
  /* tool state_vector cs enum CTRL */
  logic [1:0] cs = 2'd0;
  logic [1:0] /* tool enum CTRL */ ns;
  parameter [1:0] /* tool enum CTRL */ IDLE = 0, RUN = 1, DONE = 2, ERR = 3;
  always_ff @(posedge clk) cs <= ns;
  always_comb
    case (cs)
      IDLE: ns = go ? RUN : IDLE;
      RUN: ns = DONE;
      default: ns = IDLE;
    endcase
endmodule

module pack;
  logic [7:4] r = 4'h0;
  logic a = 0, b = 0;
  // tool state_vector r[6:5] field enum F
  localparam [1:0] /* tool enum F */ F0 = 0, F3 = 3;
  // tool state_vector {a, b} pair enum P
  localparam [1:0] /* tool enum P */ P1 = 1, P2 = 2;
endmodule

module tb;
  logic clk = 0, go = 0;
  ctrl c(clk, go);
  pack p();
  always #5 clk = ~clk;
  initial begin
    $fsm_objects;
    #2 go = 1;
    #10 go = 0;
    p.r = 4'b0110;
    {p.a, p.b} = 2'b01;
    #20 $fsm_counts(1);
    $fsm_control;
    go = 1;
    #20 $fsm_counts(2);
    $fsm_files;
    $fsm_counts(3);
    $finish;
  end
endmodule
