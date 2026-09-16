`timescale 1ns/1ps
module tb;
  integer value;
  function automatic void change(ref integer x);
    x = 9;
  endfunction
  initial begin
    value = 1;
    change(value);
    $display("CHECK: value=%0d", value);
    $finish(0);
  end
endmodule
