`timescale 1ns/1ps
module tb;
  parameter MP = 3;
  initial begin : b
    localparam K = 5;
    localparam [7:0] P = K + MP;
    localparam real R = 1.5;
    begin : c localparam K = 30; $display("%0d %0d %0.1f", K, P, R); $show(K); end
  end
  function int f(int x); localparam FK = 2; return x * FK; endfunction
  initial $display("%0d", f(3));
  initial fork : fk localparam FF = 9; $display("%0d", FF); join
endmodule
