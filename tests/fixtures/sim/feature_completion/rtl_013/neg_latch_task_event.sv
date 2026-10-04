// IEEE 1800-2009 9.2.2.3: a task called from always_latch cannot block on events.
module tb;
  logic c, d, q;
  task automatic t(); @(posedge c); endtask
  always_latch begin t(); if (d) q = d; end
  initial $finish(0);
endmodule
