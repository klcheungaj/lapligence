`timescale 1ns/1ps
module tb;
  event e;
  integer stage;
  initial begin
    stage = 0;
    ->> e;
    stage = 1;
  end
  initial begin
    @(e);
    $display("CHECK: stage=%0d", stage);
  end
  initial begin
    #1 $finish(0);
  end
endmodule
