// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/selectors.sv
// SIM-039: assertion-control selectors name assertions through generate
// blocks, named procedural blocks and subroutines (IEEE 1800-2009 20.11:
// "Subsequent arguments specify which scopes of the model to control"). Each
// assertion fails at every tick (5, 15, 25, 35); the controls at the
// negedges switch single assertions and whole scopes off and on.
module tb;
  logic clk = 1'b0;
  logic a = 1'b0;

  always #5 clk = ~clk;

  if (1) begin : g
    ga: assert property (@(posedge clk) a) else $display("%0t ga", $time);
  end

  always @(posedge clk) begin : blk
    pa: assert property (@(posedge clk) a) else $display("%0t pa", $time);
  end

  task automatic probe;
    ia: assert (a) else $display("%0t ia", $time);
  endtask

  always @(posedge clk) probe();

  initial begin
    @(negedge clk);                 // before tick 15
    $assertoff(0, tb.g.ga, tb.blk.pa);
    @(negedge clk);                 // before tick 25
    $asserton(0, tb.g);
    $assertfailoff(0, tb.probe.ia);
    @(negedge clk);                 // before tick 35
    $asserton(0, tb.blk);
    $assertfailon(0, tb.probe.ia);
    @(negedge clk);
    $finish;
  end
endmodule
