// IEEE 1800-2009 11.4.14: a stream larger than its fixed-size target is an
// error. A runtime `with` selection learns its size when evaluated, so an
// oversized descriptor stream fails at run time before any write.
module tb;
  localparam int N = 70000;
  logic [15:0] src [N];
  logic [15:0] rot [N];
  int i;
  initial begin
    i = 0;
    rot = {>>{src with [i : N-1], src with [0 : 1]}};
    $display("unreachable %h", rot[0]);
    $finish(0);
  end
endmodule
