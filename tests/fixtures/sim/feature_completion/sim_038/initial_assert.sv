// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_038/initial_assert.sv
// SIM-038: a concurrent assertion that is the whole body of an initial
// procedure is queued once and begins a single evaluation attempt at the
// first leading clock event (16.15.6, F.5.3.1); the static copy `s1` starts
// an attempt on every tick. Ticks 1..4 are the posedges at 5, 15, 25, 35.
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0;
  // Bit 3 is tick 1, bit 0 is tick 4.
  logic [3:0] v_a = 4'b1110, v_b = 4'b0100;

  initial i1: assert property (@(posedge clk) a ##1 b)
    $display("%0t i1 pass", $time); else $display("%0t i1 fail", $time);
  initial i2: assert property (@(posedge clk) a |=> b)
    $display("%0t i2 pass", $time); else $display("%0t i2 fail", $time);
  s1: assert property (@(posedge clk) a ##1 b)
    $display("%0t s1 pass", $time); else $display("%0t s1 fail", $time);

  initial begin
    for (int i = 0; i < 4; i++) begin
      a = v_a[3 - i];
      b = v_b[3 - i];
      #5 clk = 1'b1;
      #5 clk = 1'b0;
    end
    $finish;
  end
endmodule
