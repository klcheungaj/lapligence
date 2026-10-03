// IEEE 1800-2009 10.4.2, 11.4.14.4: a queued unpack publishes nothing at
// issue, so a selector that reads a target unpacked to its left by the same
// nonblocking assignment has no defined value; the owner policy rejects it.
module tb;
  logic [7:0] q [0:3];
  int j;
  initial begin
    {>>{j, q with [j +: 2]}} <= {32'd1, 16'h5566};
    #1 $finish;
  end
endmodule
