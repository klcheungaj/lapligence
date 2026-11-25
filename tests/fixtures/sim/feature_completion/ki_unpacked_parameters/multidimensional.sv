`timescale 1ns/1ps
module a1;
  parameter int A2 [2][3] = '{'{1,2,3},'{4,5,6}};
  int i;
  initial begin #0;
    $display("%0d %0d", A2[1][0], A2[0][2]);
    for (i = 0; i < 2; i++) $display("%0d", A2[i][i+1]);
  end
endmodule
module a2;
  parameter logic [3:0][7:0] P [2] = '{32'h01020304, 32'h05060708};
  int i = 1;
  initial #10 $display("%0h %0h %0h %0h", P[1], P[1][2], P[i][i], P[i][i][3:0]);
endmodule
module a3;
  parameter int A2 [2][3] = '{'{1,2,3},'{4,5,6}};
  int r [3]; int i = 1; int a [2][3];
  initial begin #20;
    r = A2[1]; $display("%p", r);
    r = A2[i]; $display("%p", r);
    a = A2; $display("%p", a);
    $display("%p", A2);
    foreach (A2[x,y]) $write("%0d ", A2[x][y]); $display;
  end
endmodule
module tb;
  a1 u_a1 ();
  a2 u_a2 ();
  a3 u_a3 ();
endmodule
