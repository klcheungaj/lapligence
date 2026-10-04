// IEEE 1800-2009 9.2.2.4: wait is a blocking timing control, illegal in always_ff.
module tb;
  logic c, d, q;
  always_ff @(posedge c) begin wait (d); q <= d; end
  initial $finish(0);
endmodule
