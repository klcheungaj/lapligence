`timescale 1ns/1ps
module b1;
  parameter string SA [2] = '{"ab", "cde"};
  string s; int i = 1;
  initial begin #0;
    $display("%s %s", SA[0], SA[1]);
    $display("%s", SA[i]);
    s = {SA[0], SA[i]};
    $display("%s", s);
    foreach (SA[k]) $display("%s", SA[k]);
    if (SA[i] == "cde") $display("eq");
  end
endmodule
module b2;
  parameter real RA [2] = '{1.5, 2.5};
  int i = 1; real r;
  initial begin #10;
    $display("%0.2f %0.2f", RA[0], RA[1]);
    $display("%0.2f", RA[i]);
    r = RA[i] * 2; $display("%0.2f", r);
    foreach (RA[k]) $display("%0.2f", RA[k]);
  end
endmodule
module b3;
  parameter real RA [2][2] = '{'{1.5, 2.5}, '{3.5, 4.5}};
  parameter string SA [2][2] = '{'{"a","b"},'{"c","d"}};
  int i = 1;
  initial #20 $display("%0.2f %0.2f %s %s", RA[1][0], RA[i][i], SA[0][1], SA[i][0]);
endmodule
module e1;
  initial begin : b
    localparam string SA [2] = '{"ab", "cde"};
    localparam int A2 [2][2] = '{'{1,2},'{3,4}};
    automatic int i = 1;
    #30 $display("%s %0d %s %0d", SA[1], A2[1][1], SA[i], A2[i][i]);
  end
endmodule
module tb;
  b1 u_b1 ();
  b2 u_b2 ();
  b3 u_b3 ();
  e1 u_e1 ();
endmodule
