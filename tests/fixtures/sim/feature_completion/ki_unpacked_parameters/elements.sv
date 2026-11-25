`timescale 1ns/1ps
package p;
  parameter string NAMES [3:1] = '{"three", "two", "one"};
  parameter int NEG [-1:1] = '{10, 20, 30};
  typedef enum logic [1:0] {A, B, C} e;
  parameter e ES [2] = '{B, C};
  typedef struct packed { logic [3:0] hi; logic signed [3:0] lo; } pk;
  parameter pk PS [2] = '{8'h1f, 8'h2e};
  parameter logic [7:0] XS [2] = '{8'hx1, 8'bz};
  parameter logic signed [7:0] SG [2] = '{-5, 7};
  parameter real T [2] = '{1.5, 2.25};
  parameter shortreal SR [2] = '{0.5, 0.75};
endpackage
module g1_sub; parameter int K [2][2] = '{'{1,2},'{3,4}}; parameter string KS [2] = '{"k0","k1"}; endmodule
module g1;
  import p::*;
  g1_sub u ();
  int i; logic [1:0] x; string s;
  initial begin #0;
    for (i = 1; i <= 3; i++) $display("%s %s", NAMES[i], p::NAMES[4-i]);
    for (i = -1; i <= 1; i++) $display("%0d", NEG[i]);
    i = 1;
    $display("%0d %0d %s", ES[i], ES[0], "C");
    $display("%h %h %0d %0d", PS[1].hi, PS[i].hi, PS[i].lo, PS[0].lo);
    $display("%b %b %b", XS[0], XS[i], XS[0][3:0]);
    $display("%0d %0d %0d", SG[0], SG[i-1], SG[i-1] + 1);
    $display("%0.3f %0.3f %0.3f %0.3f", T[i], T[0], SR[i], SR[0]);
    $display("%0d %0d %s %s", u.K[1][0], u.K[i][i], u.KS[0], u.KS[i]);
    $display("%0d %0d %0d", SG[i][7], SG[0][3:0], SG[i-1][1 +: 3]);
    x = 2'bx1; $display("%0d %0.1f", T[x] > 0, T[x]);
    i = 5; $display("%0d %0d %b", NEG[i], NEG[-i], XS[i]);
    $display("%0d %0d", $size(u.K), $bits(u.K));
    case (NAMES[2]) "two": $display("case"); default: $display("no"); endcase
  end
endmodule
module tb;
  g1 u_g1 ();
endmodule
