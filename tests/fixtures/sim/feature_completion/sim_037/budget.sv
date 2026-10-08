// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_037/budget.sv
// SIM-037 A03: every clock starts an attempt whose `##[1:$] b` obligation
// stays pending, so live sequence threads grow by a fixed amount per tick.
// With a small LLG_SEQUENCE_THREAD_LIMIT the run stops with a reported
// execution error instead of dropping attempts; with the default it ends.
module tb;
  logic clk = 1'b0;
  logic a = 1'b1;
  logic b = 1'b0;
  int t = 0;
  p: assert property (@(posedge clk) a |-> ##[1:$] b) else $display("P fail t=%0d", t);
  initial begin
    for (int k = 1; k <= 200; k++) begin
      #5 clk = 1'b1;
      #5 clk = 1'b0;
      t = k;
    end
    $display("done t=%0d", t);
    $finish;
  end
endmodule
