`timescale 1ns/1ns
module tb;
  initial begin
    $display("CHECK: literal=%0.1f", 1.55ns);
    $finish(0);
  end
endmodule
