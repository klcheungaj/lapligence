// An application that only registers a system task at start-up: the task
// handle works before any design object is reached, and handles, callbacks
// and puts made later from the task behave as usual.
`timescale 1ns/1ns
module tb;
  logic [3:0] a = 0;
  wire [3:0] w;
  assign w = a;
  initial begin
    #1 $lazy_probe;
    #1 $display("t=%0t a=%0d w=%0d", $time, a, w);
    a = 9;
    #1 $display("t=%0t a=%0d w=%0d", $time, a, w);
    $finish(0);
  end
endmodule
