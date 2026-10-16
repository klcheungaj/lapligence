// Decision S99-D4: an instance queued after the Observed region of its
// clock's time step (by a process woken by a clocking block event, or by
// program code in the Reactive region) matures when the scheduler next
// reaches the Observed region of the same time step and begins its attempt
// there, at that clock tick, with that tick's sampled values.
//
// IEEE 1800-2009 16.15.6 (SystemVerilog-1800-2009.txt L26757-26762): "In the
// Observed region of each simulation time step, each pending procedural
// assertion instance that is currently present in a procedural assertion
// queue shall mature ... When a pending procedural assertion instance
// matures, if the current time step is one that corresponds to that
// assertion instance's leading clocking event, an evaluation attempt of the
// assertion begins immediately within the Observed region."
// l1 is queued on the clocking block event at every posedge clk and l2 by
// the program at negedge clk 10 and 20; a becomes 1 at 12.
`timescale 1ns/1ns
module tb;
  logic clk = 0;
  logic a = 0;
  always #5 clk = ~clk;
  clocking cb @(posedge clk); endclocking
  always @(cb) begin
    l1: assert property (@(posedge clk) a) $display("%0t l1 pass", $time); else $display("%0t l1 fail", $time);
  end
  initial #12 a = 1;
  initial #27 $finish;
  program automatic prg;
    initial begin
      repeat (2) begin
        @(negedge clk);
        l2: assert property (@(negedge clk) a) $display("%0t l2 pass", $time); else $display("%0t l2 fail", $time);
      end
      // Ending every program calls $finish (24.3); stay alive past tb's.
      #40;
    end
  endprogram
  prg u_prg();
endmodule
