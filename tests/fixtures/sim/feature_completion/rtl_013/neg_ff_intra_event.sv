// IEEE 1800-2009 9.2.2.4: an intra-assignment event is a second event control.
module tb;
  logic c, d, q;
  always_ff @(posedge c) q = @(negedge c) d;
  initial $finish(0);
endmodule
