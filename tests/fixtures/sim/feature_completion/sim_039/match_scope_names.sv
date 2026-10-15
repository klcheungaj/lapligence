// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/match_scope_names.sv
// SIM-039 follow-up: %m inside an assertion's action block and in a system task attached to a sequence match names the assertion's scope, never a hidden host process (IEEE 1800-2009 21.2.1.6, 16.15).
module tb;
  logic clk = 0, a = 1;
  sequence s;
    (a, $display("match %m"));
  endsequence
  cover property (@(posedge clk) s) $display("action %m");
  if (1) begin : blk
    lab: cover property (@(posedge clk) a) $display("labeled-action %m");
  end
  initial begin : ib
    pc: cover property (@(posedge clk) s) begin : pact
      $display("procedural-action %m");
    end
    #1 clk = 1;
    #1 $finish;
  end
endmodule
