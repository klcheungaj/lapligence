// IEEE 1800-2009 14.4: an input skew of #d samples the value d time units
// before the clocking event. Two blocks read one source with skews #3 and #7;
// the slow block first fires long after the fast one, so the source's history
// must keep the deepest skew while older slots are released.
module tb;
  timeunit 1ns;
  timeprecision 100ps;

  logic fast = 0;
  logic slow = 0;
  logic [15:0] data = 0;

  clocking cb_fast @(posedge fast);
    input #3 data;
  endclocking

  clocking cb_slow @(posedge slow);
    input #7 data;
  endclocking

  // data becomes k at time k - 0.5, so it equals t at every integer t >= 0.
  initial begin
    #0.5 data = 1;
    forever #1 data = data + 1;
  end

  always #1 fast = ~fast;

  initial begin
    #101 slow = 1;
    forever #25 slow = ~slow;
  end

  always @(cb_slow) $display("t=%0d slow=%0d fast=%0d", $time, cb_slow.data, cb_fast.data);

  initial #260 $finish;
endmodule
