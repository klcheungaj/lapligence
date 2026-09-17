`timescale 1ns/1ps
module tb;
  wire a, b;
  alias a = b;
  assign a = 1'b1;
  initial begin
    #1 $display("CHECK: b=%b", b);
    $finish(0);
  end
endmodule
