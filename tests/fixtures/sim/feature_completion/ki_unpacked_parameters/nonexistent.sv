`timescale 1ns/1ps
module tb;
  parameter string SA [2] = '{"ab", "cde"};
  parameter int IA [2] = '{1, 2};
  parameter logic [3:0] LA [2][2] = '{'{1, 2}, '{3, 4}};
  parameter real RA [2] = '{1.5, 2.5};
  int i = 2, j = 0;
  logic [1:0] x = 2'bx1;
  initial begin
    $display("<%s> <%s> %0d %0d %b %0.1f", SA[i], SA[-i], IA[i], IA[-i], LA[i][0], RA[i]);
    $display("%b %b %b %b", LA[0][i], LA[j][i], LA[j][-1], LA[i][j]);
    $display("<%s> %0d %b %0.1f", SA[x], IA[x], LA[x][0], RA[x]);
  end
endmodule
