// ADV-017 FSM state coverage (IEEE 1800-2009 40.2.2, 40.3.2, 40.4):
// every pragma form of 40.4.1-40.4.7 and the $coverage_* functions for
// SV_COV_FSM_STATE. Derivations: readme.md.
`timescale 1ns/1ns

// 40.4.1 with a separate enum pragma; 40.4.5 cs and ns in one declaration.
module ctrl(input logic clk, input logic rst, input logic go);
  /* tool state_vector cs */
  logic [1:0] /* tool enum CTRL */ cs, ns, other;
  parameter [1:0] /* tool enum CTRL */
    IDLE = 0,
    RUN = 1,
    DONE = 2,
    ERR = 3;
  always_ff @(posedge clk or posedge rst)
    if (rst) cs <= IDLE;
    else cs <= ns;
  always_comb begin
    other = 2'b00;
    case (cs)
      IDLE: ns = go ? RUN : IDLE;
      RUN: ns = DONE;
      default: ns = IDLE;
    endcase
  end
endmodule

// The 40.4.8 example: one-line comments, enum named in the state_vector
// pragma, untyped parameters.
module m3(input logic [1:0] sel);
  reg [31:0] cs;
  reg [31:0] /* tool enum MY_FSM */ ns;
  // tool state_vector cs enum MY_FSM
  parameter // tool enum MY_FSM
    p1 = 10,
    p2 = 11,
    p3 = 12;
  always_comb cs = 10 + sel;
  always_comb ns = cs;
endmodule

// An ANSI output port, a part-select (40.4.2), a concatenation (40.4.3),
// a continuously assigned net and localparam states.
module misc(output logic [2:0] /* tool enum Q */ q);
  // tool state_vector q
  localparam [2:0] // tool enum Q
    Q0 = 3'b001,
    Q1 = 3'b010,
    Q2 = 3'b100;
  logic [7:0] r;
  /* tool state_vector r[5:4] part enum E3 */
  localparam /* tool enum E3 */ X0 = 0, X1 = 3;
  logic a, b;
  /* tool state_vector {a, b} pair enum E4 */
  parameter /* tool enum E4 */ Y0 = 1, Y1 = 2;
  wire [1:0] /* tool enum W */ w;
  /* tool state_vector w */
  parameter [1:0] /* tool enum W */ W0 = 1, W1 = 2;
  assign w = {a, b};
  initial begin
    q = Q0;
    r = 8'h00;
    a = 0;
    b = 0;
  end
endmodule

module empty_unit;
endmodule

module tb;
  logic clk = 0, rst = 1, go1 = 0, go2 = 0;
  logic [1:0] sel = 0;
  ctrl c1(clk, rst, go1);
  ctrl c2(clk, rst, go2);
  m3 u3(sel);
  misc u4(.q());
  empty_unit e();
  always #5 clk = ~clk;

  task automatic show(input string label);
    $display("%s: tb %0d/%0d c1 %0d/%0d c2 %0d ctrl %0d/%0d u3 %0d/%0d u4 %0d/%0d", label,
             $coverage_get(`SV_COV_FSM_STATE, `SV_COV_HIER, tb),
             $coverage_get_max(`SV_COV_FSM_STATE, `SV_COV_HIER, tb),
             $coverage_get(`SV_COV_FSM_STATE, `SV_COV_MODULE, c1),
             $coverage_get_max(`SV_COV_FSM_STATE, `SV_COV_MODULE, c1),
             $coverage_get(`SV_COV_FSM_STATE, `SV_COV_MODULE, c2),
             $coverage_get(`SV_COV_FSM_STATE, `SV_COV_MODULE, "ctrl"),
             $coverage_get_max(`SV_COV_FSM_STATE, `SV_COV_MODULE, "ctrl"),
             $coverage_get(`SV_COV_FSM_STATE, `SV_COV_MODULE, u3),
             $coverage_get_max(`SV_COV_FSM_STATE, `SV_COV_MODULE, u3),
             $coverage_get(`SV_COV_FSM_STATE, `SV_COV_MODULE, u4),
             $coverage_get_max(`SV_COV_FSM_STATE, `SV_COV_MODULE, u4));
  endtask

  initial begin
    $display("check: tb hier %0d tb module %0d e %0d root %0d",
             $coverage_control(`SV_COV_CHECK, `SV_COV_FSM_STATE, `SV_COV_HIER, tb),
             $coverage_control(`SV_COV_CHECK, `SV_COV_FSM_STATE, `SV_COV_MODULE, tb),
             $coverage_control(`SV_COV_CHECK, `SV_COV_FSM_STATE, `SV_COV_HIER, e),
             $coverage_control(`SV_COV_CHECK, `SV_COV_FSM_STATE, `SV_COV_HIER, $root));
    $display("empty: start %0d stop %0d reset %0d get %0d max %0d",
             $coverage_control(`SV_COV_START, `SV_COV_FSM_STATE, `SV_COV_HIER, e),
             $coverage_control(`SV_COV_STOP, `SV_COV_FSM_STATE, `SV_COV_HIER, e),
             $coverage_control(`SV_COV_RESET, `SV_COV_FSM_STATE, `SV_COV_HIER, e),
             $coverage_get(`SV_COV_FSM_STATE, `SV_COV_HIER, e),
             $coverage_get_max(`SV_COV_FSM_STATE, `SV_COV_HIER, e));
    $display("bad scope: %0d %0d", $coverage_get(`SV_COV_FSM_STATE, 12, tb),
             $coverage_get(`SV_COV_FSM_STATE, `SV_COV_MODULE, "nowhere"));
    #1 show("t1");
    #1 sel = 1;
    #1 sel = 3;
    #1 sel = 2;
    #2 show("t6");
    #1 $display("stop c2 %0d", $coverage_control(`SV_COV_STOP, `SV_COV_FSM_STATE,
                                                  `SV_COV_MODULE, c2));
    #1 rst = 0;
    go1 = 1;
    go2 = 1;
    #2 u4.r[4] = 1;
    #1 u4.r[5] = 1;
    #1 {u4.a, u4.b} = 2'b01;
    #1 {u4.a, u4.b} = 2'b10;
    #1 force u4.q = 3'b100;
    #1 release u4.q;
    #1 show("t16");
    #1 $display("start c2 %0d", $coverage_control(`SV_COV_START, `SV_COV_FSM_STATE,
                                                   `SV_COV_MODULE, c2));
    go2 = 0;
    #9 show("t26");
    #1 $display("reset c1 %0d", $coverage_control(`SV_COV_RESET, `SV_COV_FSM_STATE,
                                                   `SV_COV_MODULE, c1));
    show("t27");
    #9 show("t36");
    $display("save %0d", $coverage_save(`SV_COV_FSM_STATE, "fsm_coverage.db"));
    $display("reset tb %0d", $coverage_control(`SV_COV_RESET, `SV_COV_FSM_STATE,
                                               `SV_COV_HIER, tb));
    show("reset");
    $display("merge %0d assertion merge %0d",
             $coverage_merge(`SV_COV_FSM_STATE, "fsm_coverage.db"),
             $coverage_merge(`SV_COV_ASSERTION, "fsm_coverage.db"));
    show("merged");
    $finish;
  end
endmodule
