// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/action_controls.sv
// SIM-039 A02: the six IEEE 1800-2009 20.12 action-control tasks at selected
// scopes (a module instance, a hierarchical assertion name, a bare assertion
// name, a cover and the whole design) with vacuous and nonvacuous successes
// and failures. Ticks are the posedges at 5, 15, ..., 85; each control task
// and each stimulus change happens at the negedge before the tick it affects.
// `$finish(2)` prints the result counters, which action controls must not
// change. Derivation: readme.md.
module unit #(parameter string NAME = "") (input logic clk, input logic a, input logic b);
  chk: assert property (@(posedge clk) a |-> b)
    $display("%0t %s pass", $time, NAME);
  else
    $display("%0t %s fail", $time, NAME);
endmodule

module tb;
  logic clk = 1'b0;
  logic a = 1'b0;
  logic b = 1'b0;

  unit #("u1") u1(clk, a, b);
  unit #("u2") u2(clk, a, b);

  top_chk: assert property (@(posedge clk) a |-> b)
    $display("%0t top pass", $time);
  else
    $display("%0t top fail", $time);

  nd: assert property (@(posedge clk) a |-> b);

  cov: cover property (@(posedge clk) a ##0 b)
    $display("%0t cov hit", $time);

  always #5 clk = ~clk;

  initial begin
    @(negedge clk);                     // t=10, before tick 15
    $assertvacuousoff(0, tb.u1);
    @(negedge clk);                     // t=20, before tick 25
    $assertpassoff(0, tb.u2.chk);
    a = 1'b1; b = 1'b1;
    @(negedge clk);                     // t=30, before tick 35
    $assertfailoff(0, top_chk, tb.u1.chk);
    b = 1'b0;
    @(negedge clk);                     // t=40, before tick 45
    $assertnonvacuouson(0, tb.u2);
    b = 1'b1;
    @(negedge clk);                     // t=50, before tick 55
    a = 1'b0;
    @(negedge clk);                     // t=60, before tick 65
    $assertpasson;
    $assertfailon;
    $assertfailoff(0, nd);
    $assertpassoff(0, cov);
    a = 1'b1; b = 1'b0;
    @(negedge clk);                     // t=70, before tick 75
    b = 1'b1;
    @(negedge clk);                     // t=80, before tick 85
    a = 1'b0;
    @(negedge clk);                     // t=90
    $finish(2);
  end
endmodule
