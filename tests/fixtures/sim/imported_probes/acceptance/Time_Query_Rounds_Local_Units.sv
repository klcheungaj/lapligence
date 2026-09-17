`timescale 10ns/1ns
module tb;
  initial begin
    #1.55 $display("CHECK: first=%0d", $time);
    #1.55 $display("CHECK: second=%0d", $time);
    $finish(0);
  end
endmodule
