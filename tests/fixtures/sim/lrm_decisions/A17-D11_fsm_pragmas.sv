// Decision A17-D11: llg recognizes an FSM only from the IEEE 1800-2009
// 40.4 pragmas and checks them strictly. An FSM needs a `tool state_vector`
// pragma in the module that declares its signals, an enumeration name (in
// that pragma or in a `tool enum` pragma right after the signal's type and
// range) and a parameter or localparam declaration with the same `tool
// enum` pragma listing its states; the state values must be distinct and
// fit the state vector (at most 64 bits). The FSM is named after its state
// signal, or by the name the part-select and concatenation forms give it.
// In a `tool enum` declaration of several signals the first is the state
// (when the state_vector pragma names it) and the next one the next state
// (40.4.5). A malformed pragma, one that names no signal of its module, a
// state vector without states, an enumeration that names no FSM and a
// state that does not fit are errors: no FSM is silently left uncovered.
// Here every FSM is recognized, so $coverage_get_max reports the number of
// states of each instance: ctrl 4, m3 3 (the 40.4.8 example), and each
// state vector of `two` 2.
//
// IEEE 1800-2009 40.4 (SystemVerilog-1800-2009.txt L66380-L66382):
// "This standard does not attempt to describe or require any specific
// automatic FSM recognition mechanisms. However, this standard does
// prescribe a means by which nonautomatic FSM extraction occurs."; 40.4.1
// L66392-L66397: "/* tool state_vector signal_name */ ... This pragma
// needs to be specified inside the module definition where the signal is
// declared."; L66399: "Another pragma is also required that specifies an
// enumeration name for the FSM."; 40.4.5 L66464-L66465: "the tool assumes
// signal cs holds the current state and signal ns holds the next state. It
// assumes nothing about signal nonstate."; 40.4.6 L66469-L66470: "The
// possible states of the FSM can also be specified with a pragma that
// includes the following enumeration name". llg choice: the states are
// required (llg does not extract them automatically) and localparam is
// accepted like parameter.
module ctrl;
  /* tool state_vector cs */
  logic [1:0] /* tool enum CTRL */ cs, ns, nonstate;
  parameter [1:0] /* tool enum CTRL */ IDLE = 0, RUN = 1, DONE = 2, ERR = 3;
endmodule

module m3;
  reg [31:0] cs;
  reg [31:0] /* tool enum MY_FSM */ ns;
  reg [31:0] clk;
  reg [31:0] rst;
  // tool state_vector cs enum MY_FSM
  parameter // tool enum MY_FSM
    p1 = 10,
    p2 = 11,
    p3 = 12;
endmodule

module two;
  logic [7:0] r;
  logic a, b;
  /* tool state_vector r[3:2] low enum L */
  localparam /* tool enum L */ L0 = 0, L2 = 2;
  /* tool state_vector {a, b} ab enum AB */
  localparam [1:0] /* tool enum AB */ AB1 = 1, AB2 = 2;
endmodule

module tb;
  ctrl c();
  m3 u();
  two t();
  initial
    $display("states: ctrl %0d m3 %0d two %0d all %0d",
             $coverage_get_max(`SV_COV_FSM_STATE, `SV_COV_MODULE, c),
             $coverage_get_max(`SV_COV_FSM_STATE, `SV_COV_MODULE, u),
             $coverage_get_max(`SV_COV_FSM_STATE, `SV_COV_MODULE, t),
             $coverage_get_max(`SV_COV_FSM_STATE, `SV_COV_HIER, tb));
endmodule
