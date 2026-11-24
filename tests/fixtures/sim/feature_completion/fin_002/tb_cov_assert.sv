// FIN-002 testbench scenario 3: a two-request arbiter checked by concurrent
// assertions (single clock and multiclock), a deferred assertion, a checker
// instance and an immediate assertion in a class method. The action blocks
// sample covergroups and update a class tally; a second covergroup samples
// the state machine on its clock. `$assertkill` cancels an attempt before
// its action block runs and a `force` of the grant makes the checker fail.
// Every expected line is derived in readme.md.
`timescale 1ns / 1ns

package arb_pkg;
  typedef enum logic [1:0] {
    IDLE,
    G0,
    G1
  } state_t;

  class tally;
    int passes[string];
    int fails[string];
    function void note(string name, bit pass);
      a_named : assert (name.len() > 0)
      else $display("tally: unnamed assertion");
      if (!passes.exists(name)) begin
        passes[name] = 0;
        fails[name]  = 0;
      end
      if (pass) passes[name]++;
      else fails[name]++;
    endfunction
  endclass
endpackage

module arb (
    input  logic       clk,
    input  logic       rst,
    input  logic [1:0] req,
    output logic [1:0] gnt
);
  import arb_pkg::*;
  state_t state, next;
  always_ff @(posedge clk)
    if (rst) state <= IDLE;
    else state <= next;
  always_comb begin
    next = state;
    case (state)
      IDLE:
      if (req[0]) next = G0;
      else if (req[1]) next = G1;
      G0: if (!req[0]) next = IDLE;
      G1: if (!req[1]) next = IDLE;
      default: next = IDLE;
    endcase
  end
  assign gnt = {state == G1, state == G0};
endmodule

// CHECKER_AS_MODULE gives the same assertions in a module, for tools
// without checkers; the output is the same.
`ifdef CHECKER_AS_MODULE
module grant_chk (
    input logic clk,
    input logic rst,
    input logic [1:0] req,
    input logic [1:0] gnt
);
`else
checker grant_chk (
    logic clk,
    logic rst,
    logic [1:0] req,
    logic [1:0] gnt
);
`endif
  a_cause :
  assert property (@(posedge clk) disable iff (rst) $rose(gnt[0]) |-> $past(req[0]))
  else $display("[%0t] chk a_cause failed", $time);
  a_mutex :
  assert property (@(posedge clk) !(gnt[0] && gnt[1]))
  else $display("[%0t] chk a_mutex failed", $time);
  c_both :
  cover property (@(posedge clk) req == 2'b11) $display("[%0t] chk both requested", $time);
`ifdef CHECKER_AS_MODULE
endmodule
`else
endchecker
`endif

module tb;
  import arb_pkg::*;
  logic clk = 1'b0;
  logic sclk = 1'b0;
  logic rst = 1'b1;
  logic [1:0] req = 2'b00;
  logic [1:0] gnt;
  logic ack = 1'b0;
  tally score = new();

  always #5 clk = ~clk;
  always #7 sclk = ~sclk;

  arb dut (
      .clk(clk),
      .rst(rst),
      .req(req),
      .gnt(gnt)
  );
  grant_chk chk (
      clk,
      rst,
      req,
      gnt
  );

  // The slow domain acknowledges any grant it sees.
  always_ff @(posedge sclk) ack <= |gnt;

  covergroup cg_result with function sample (bit pass, bit [1:0] who);
    option.per_instance = 1;
    cp_pass: coverpoint pass {bins ok = {1}; bins bad = {0};}
    cp_who: coverpoint who {bins none = {0}; bins g0 = {1}; bins g1 = {2};}
    x: cross cp_pass, cp_who;
  endgroup

  covergroup cg_ack with function sample (bit pass);
    option.per_instance = 1;
    option.at_least = 2;
    cp: coverpoint pass {bins ok = {1}; bins bad = {0};}
  endgroup

  covergroup cg_fsm @(posedge clk iff !rst);
    option.per_instance = 1;
    cp_state: coverpoint dut.state {
      bins idle = {IDLE};
      bins g0 = {G0};
      bins g1 = {G1};
      bins idle_g0 = (IDLE => G0);
      bins g0_g1 = (G0 => G1);
      bins g1_idle = (G1 => IDLE);
    }
  endgroup

  cg_result cov = new();
  cg_ack cov_ack = new();
  cg_fsm cov_fsm = new();

  a_g0 :
  assert property (@(posedge clk) disable iff (rst) $rose(req[0]) && !gnt[1] |-> ##[1:2] gnt[0]) begin
    cov.sample(1'b1, 2'd1);
    score.note("a_g0", 1'b1);
    $display("[%0t] a_g0 passed", $time);
  end else begin
    cov.sample(1'b0, 2'd1);
    score.note("a_g0", 1'b0);
    $display("[%0t] a_g0 failed", $time);
  end

  a_g1 :
  assert property (@(posedge clk) disable iff (rst) $rose(req[1]) |-> ##[1:2] gnt[1]) begin
    cov.sample(1'b1, 2'd2);
    score.note("a_g1", 1'b1);
    $display("[%0t] a_g1 passed", $time);
  end else begin
    cov.sample(1'b0, 2'd2);
    score.note("a_g1", 1'b0);
    $display("[%0t] a_g1 failed", $time);
  end

  // Multiclock: a new grant is acknowledged one slow tick after the slow
  // domain first sees it.
  a_ack :
  assert property (@(posedge clk) disable iff (rst) $rose(|gnt) |=> @(posedge sclk) ##1 ack) begin
    cov_ack.sample(1'b1);
    score.note("a_ack", 1'b1);
    $display("[%0t] a_ack passed", $time);
  end else begin
    cov_ack.sample(1'b0);
    score.note("a_ack", 1'b0);
    $display("[%0t] a_ack failed", $time);
  end

  always_comb begin
    a_req :
    assert #0 (!(req == 2'b11 && gnt == 2'b00))
    else $display("[%0t] deferred a_req: both requested while idle", $time);
  end

  initial begin
    int covered;
    int total;
    // Pass actions run for nonvacuous successes only. NO_VACUOUS_CONTROL
    // leaves the call out, for tools that never run them on a vacuous
    // success and lack the task.
`ifndef NO_VACUOUS_CONTROL
    $assertvacuousoff(0);
`endif
    @(negedge clk) rst = 1'b0;
    @(negedge clk) req = 2'b01;
    @(negedge clk);
    @(negedge clk) req = 2'b00;
    @(negedge clk) req = 2'b11;
    @(negedge clk) req = 2'b10;
    @(negedge clk);
    @(negedge clk);
    @(negedge clk) req = 2'b00;
    @(negedge clk) req = 2'b01;
    #7 $assertkill(0, tb);
    $display("[%0t] assertions killed", $time);
    @(negedge clk) req = 2'b00;
    #8 $asserton(0, tb);
    $display("[%0t] assertions on", $time);
    @(negedge clk) req = 2'b10;
    @(negedge clk) req = 2'b00;
    @(negedge clk);
    #2 force gnt = 2'b11;
    #10 release gnt;
    #18;
    $display("[%0t] result cov=%0.2f inst=%0.2f pass=%0.2f who=%0.2f cross=%0.2f", $time,
             cov.get_coverage(), cov.get_inst_coverage(), cov.cp_pass.get_coverage(),
             cov.cp_who.get_inst_coverage(), cov.x.get_coverage());
    void'(cov.cp_who.get_inst_coverage(covered, total));
    $display("who bins %0d/%0d", covered, total);
    void'(cov.x.get_inst_coverage(covered, total));
    $display("cross bins %0d/%0d", covered, total);
    $display("ack cov=%0.2f fsm cov=%0.2f all=%0.2f", cov_ack.get_coverage(),
             cov_fsm.get_inst_coverage(), $get_coverage());
    foreach (score.passes[name])
      $display("%s: passes=%0d fails=%0d", name, score.passes[name], score.fails[name]);
    $finish;
  end
endmodule
