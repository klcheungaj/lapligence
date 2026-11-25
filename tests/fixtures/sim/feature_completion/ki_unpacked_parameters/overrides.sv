`timescale 1ns/1ps
module d1_sub #(parameter string SA [2] = '{"x","y"}, parameter real R [2] = '{0.0, 1.0}, parameter int M [2][2] = '{'{1,2},'{3,4}});
  initial #0 $display("%s %s %0.1f %0.1f %0d", SA[0], SA[1], R[0], R[1], M[1][0]);
endmodule
module d1;
  d1_sub u ();
  d1_sub #(.SA('{"p","q"}), .R('{3.5,4.5}), .M('{'{5,6},'{7,8}})) v ();
endmodule
module f1_m;
  parameter int A [2] = '{1,2};
  typedef struct { int a; logic [7:0] b; } t;
  parameter t US = '{1, 8'hab};
  initial #10 $display("%0d %0d %0d", A[0], A[1], US.a);
endmodule
module f1;
  f1_m u(); f1_m v();
  defparam u.A = '{5,6};
  defparam v.US = '{7, 8'h1};
endmodule
module f2;
  parameter int A [2] = '{1,2};
  parameter int B [2] = A;
  parameter int C [4] = '{default: 3, 1: 7};
  typedef struct { int a; logic [7:0] b; } t;
  parameter t U = '{b: 8'h2, default: 1};
  parameter t V = U;
  int i = 1;
  initial #20 $display("%0d %0d %0d %0d %0d %0d %0d", B[1], B[i], C[1], C[i], C[3], U.a, V.b);
endmodule
module tb;
  d1 u_d1 ();
  f1 u_f1 ();
  f2 u_f2 ();
endmodule
