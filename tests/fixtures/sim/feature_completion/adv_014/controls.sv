// ADV-014 A02: disable iff and the assertion control tasks while
// evaluations of assertions with global clocking future sampled value
// functions are pending (IEEE 1800-2009 16.9.4, 16.12, 20.11, 20.12). The
// attempt interval ends at the last assertion clock tick: a $assertkill or a
// disable iff condition strictly after it does not affect the attempt, whose
// action still runs at the next global clock tick; within it they discard
// the attempt. $assertoff stops new attempts only. The action controls apply
// as they stood at the last tick, not when the delayed action runs (decision
// A14-D4). Expected lines: readme.md.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, slow = 1'b0;
  logic a = 1'b0, b = 1'b0, rst = 1'b0, rst2 = 1'b0;
  global clocking gc @(posedge clk);
  endclocking
  always #5 clk = ~clk;
  always #20 slow = ~slow;
  initial begin
    a = 1'b1;
    #22 b = 1'b1;
    #36 $assertpassoff(0, tb.pon);
    #4 begin
      $assertkill(0, tb.kin);
      rst = 1'b1;
      $assertpassoff(0, tb.pof);
      $assertpasson(0, tb.pon);
    end
    #1 rst = 1'b0;
    #7 $assertpasson(0, tb.pof);
    #10 $asserton(0, tb.kin);
    #20 rst2 = 1'b1;
    #1 rst2 = 1'b0;
    #1 begin
      $assertkill(0, tb.kaf);
      $assertoff(0, tb.off);
    end
    #20 $asserton(0, tb.off);
    #50 $finish;
  end
  // Slow clock ticks 20, 60, 100, 140; each attempt's last tick is the
  // next one, and it reports at the global clock tick 5 later.
  kin: assert property (@(posedge slow) a |=> $steady_gclk(b))
    $display("%0t kin pass", $time); else $display("%0t kin fail", $time);
  kaf: assert property (@(posedge slow) a |=> $steady_gclk(b))
    $display("%0t kaf pass", $time); else $display("%0t kaf fail", $time);
  dis: assert property (@(posedge slow) disable iff (rst) a |=> $steady_gclk(b))
    $display("%0t dis pass", $time); else $display("%0t dis fail", $time);
  dat: assert property (@(posedge slow) disable iff (rst2) a |=> $steady_gclk(b))
    $display("%0t dat pass", $time); else $display("%0t dat fail", $time);
  pof: assert property (@(posedge slow) a |=> $steady_gclk(b))
    $display("%0t pof pass", $time); else $display("%0t pof fail", $time);
  pon: assert property (@(posedge slow) a |=> $steady_gclk(b))
    $display("%0t pon pass", $time); else $display("%0t pon fail", $time);
  off: assert property (@(posedge slow) a |=> $steady_gclk(b))
    $display("%0t off pass", $time); else $display("%0t off fail", $time);
endmodule
