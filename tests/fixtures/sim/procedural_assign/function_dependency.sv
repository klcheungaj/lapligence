`timescale 1ns/1ps
module tb;
  reg q, source;
  function reg identity(input reg value);
    identity = value;
  endfunction
  initial begin
    source = 0;
    assign q = identity(source);
    #1 $display("CHECK: initial=%b", q);
    source = 1;
    #1 $display("CHECK: changed=%b", q);
    deassign q;
    $finish(0);
  end
endmodule
