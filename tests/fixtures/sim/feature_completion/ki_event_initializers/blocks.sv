// Queues and dynamic arrays of events declared in a procedural block.
`timescale 1ns/1ns
module tb; event a, b;
 initial begin : blk
  automatic event q[$];
  event d[];
  q.push_back(a); d = new[2]; d[0] = b;
  $display("%b %b %0d %b", q[0] == a, d[0] == b, q.size(), d[1] == null);
  #1 -> q[0];
  @(d[0]) $display("W %0t", $time);
  $finish;
 end
 initial begin @(a) $display("A %0t", $time); #1 -> b; end
endmodule
