// The design ends when no event remains; the continuous assignment still
// waits on `a`, as every continuous assignment does, but it cannot block
// the end of the run.
module tb;
  logic a = 1'b1;
  wire b;
  assign b = a;
  initial #1 $display("initial done");
  final $display("final b=%b", b);
endmodule
