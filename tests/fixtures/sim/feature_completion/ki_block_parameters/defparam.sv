`timescale 1ns/1ps
module sub;
  initial begin : b
    parameter P = 1;
    parameter signed [7:0] Q = P + 1;
    localparam L = Q * 2;
    #1 $display("%m P=%0d Q=%0d L=%0d", P, Q, L);
    begin : inner
      parameter R = L + 1;
      logic [R-1:0] w;
      #1 $display("%m R=%0d w=%0d", R, $bits(w));
    end
  end
endmodule
module gen #(parameter N = 3);
  genvar i;
  for (i = 0; i < N; i = i + 1) begin : g
    initial begin : b
      parameter P = i;
      #3 $display("%m P=%0d", P);
    end
    defparam b.P = 100 + i;
  end
  for (i = 0; i < N; i = i + 1) begin : h
    initial begin : b
      parameter P = i;
      #4 $display("%m P=%0d", P);
    end
  end
  defparam h[1].b.P = 77;
endmodule
module tf;
  task t(output int v);
    parameter T = 4;
    v = T;
  endtask
  function automatic int f(int x);
    parameter F = 6;
    f = F + x;
  endfunction
  int v;
  initial begin #5 t(v); $display("%m t=%0d f=%0d", v, f(1)); end
endmodule
module kinds;
  initial begin : b
    parameter P = 3;
    parameter [7:0] C = 3;
    parameter real R = 1.5;
    parameter string S = "ab";
    #6 $display("%m %0.4f %0d %0.3f %s", P, C, R, S);
  end
  final begin : fb
    parameter F = 8;
    $display("%m %0d", F);
  end
endmodule
module tb;
  parameter W = 9;
  sub s0 (); sub s1 (); sub s2 ();
  gen g0 (); gen g1 ();
  tf t0 (); tf t1 ();
  defparam s1.b.P = 20;
  defparam s2.b.Q = 300;
  defparam s2.b.inner.R = W;
  defparam g1.h[2].b.P = 55;
  defparam t1.t.T = 44;
  defparam t1.f.F = 66;
  kinds k0 (); kinds k1 ();
  defparam k1.b.P = 3.5;
  defparam k1.b.C = 300;
  defparam k1.b.R = 7;
  defparam k1.b.S = "other";
  defparam k1.fb.F = 80;
  initial #10 $finish(0);
endmodule
