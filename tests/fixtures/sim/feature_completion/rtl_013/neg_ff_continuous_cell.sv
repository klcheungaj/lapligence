// IEEE 1800-2009 6.5 and 9.2.2.4: a continuously assigned cell has no procedural writer.
module tb;
  logic [7:0] m [0:3]; logic [7:0] x; logic c;
  always_ff @(posedge c) m[2] <= x;
  assign m[2] = x;
  initial $finish(0);
endmodule
