`timescale 1ns/1ps
module sub #(parameter W = 3) ();
  int v = 4;
  initial begin : b
    localparam K = 5;
    parameter P = K * W;
    localparam int signed N = -2;
    localparam logic [7:0] B = 8'hfx;
    localparam real R = 1.5 * W;
    localparam shortreal SR = 0.1;
    localparam string S = "hello";
    localparam U = "ab";
    localparam [15:0] US = "c";
    localparam time T = 3ns;
    localparam realtime RT = 1.26ns;
    logic [P-1:0] vec;
    int arr [K];
    vec = '1;
    $display("%0d %0d %0d %b %0.3f %0.6f %s %s %s %0t %0.3f", K + v, P, N, B, R, SR, S, U, US, T, RT);
    $display("%0d %0d %0d %s", $bits(vec), $size(arr), $bits(U), {S, "!"});
    $display("%0d %0d", $bits(K), P'(v) + K);
    $display("%m");
    $display("%0d %0d", S.len(), U[3:0]);
    $display("%0d %0d", -N, N >>> 1);
  end
endmodule
module tb; sub s1(); sub #(5) s2(); endmodule
