// IEEE 1800-2009 9.2.2.4: a task called from always_ff cannot block on timing.
module tb;
  logic c, d, q;
  task automatic t(); #1; endtask
  always_ff @(posedge c) begin t(); q <= d; end
  initial $finish(0);
endmodule
