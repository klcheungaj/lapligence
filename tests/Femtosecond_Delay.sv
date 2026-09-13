`timescale 1ps/1fs
module tb;
  initial begin
    #0.001;
    $display("CHECK: elapsed=%0.3f", $realtime);
    $finish(0);
  end
endmodule
