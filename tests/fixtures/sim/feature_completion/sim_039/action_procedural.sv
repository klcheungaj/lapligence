// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/action_procedural.sv
// SIM-039 A02: action controls (IEEE 1800-2009 20.12) on immediate, deferred
// and expect assertions, selected by local name, hierarchical name and module.
// A deferred report reads the controls when it executes in the Reactive
// region (S39-D2). Derivation: readme.md.
module tb;
  logic clk = 1'b0;
  logic a = 1'b1;

  task automatic check(input logic v);
    imm: assert (v) $display("%0t imm pass", $time);
    else $display("%0t imm fail", $time);
    dfr: assert #0 (v) $display("%0t dfr pass", $time);
    else $display("%0t dfr fail", $time);
    nde: assert (v);
  endtask

  always #5 clk = ~clk;

  initial begin
    check(1'b0);
    #1 $assertfailoff(0, check.imm, tb.check.dfr);
    check(1'b0);
    #1 $assertpassoff(0, tb);
    check(1'b1);
    #1 $assertpasson(0, tb.check.imm);
    check(1'b1);
    #1 $assertfailon;
    $assertfailoff(0, check.nde);
    check(1'b0);
    #1 check(1'b0);
    $assertfailoff(0, check.dfr);
  end

  initial begin
    #10;
    // Clear the pass-action rules of the first process, including the
    // module-wide `$assertpassoff(0, tb)` that also selects e1.
    $assertpasson;
    for (int i = 0; i < 3; i++) begin
      if (i == 1) $assertpassoff(0, e1);
      if (i == 2) begin
        $assertpasson(0, e1);
        $assertfailoff(0, e1);
        a = 1'b0;
      end
      e1: expect (@(posedge clk) a) $display("%0t e1 pass", $time);
      else $display("%0t e1 fail", $time);
      $display("%0t after e1 %0d", $time, i);
    end
    $finish(2);
  end
endmodule
