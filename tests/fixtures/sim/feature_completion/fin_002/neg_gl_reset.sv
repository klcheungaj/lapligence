// FIN-002 gate-level negative: simulation reset is not simulated; the run
// stops when the model starts.
`timescale 1ns / 1ns
module tb;
  reg a;
  wire y;
  not (y, a);
  initial begin
    a = 1'b0;
    #1 $display("y=%b", y);
    $reset;
  end
endmodule
