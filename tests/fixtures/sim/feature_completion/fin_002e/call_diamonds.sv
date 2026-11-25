// Each subroutine calls the next one at two places, so `s0` reaches `s30` along
// 2^30 call paths. An argument of 200 is the only one that takes them.
`timescale 1ns/1ns
module tb;
  logic [7:0] d = 1, e = 2, y, f, r = 7, q = 9;
  wire [7:0] n;
  class c;
    int v;
  endclass
  c h = new, k = new;

  function automatic logic [7:0] s30(input logic [7:0] x); return x ^ e; endfunction
  function automatic logic [7:0] s29(input logic [7:0] x); return x == 200 ? s30(x) ^ s30(x + 1) : x + e; endfunction
  function automatic logic [7:0] s28(input logic [7:0] x); return x == 200 ? s29(x) ^ s29(x + 1) : x + e; endfunction
  function automatic logic [7:0] s27(input logic [7:0] x); return x == 200 ? s28(x) ^ s28(x + 1) : x + e; endfunction
  function automatic logic [7:0] s26(input logic [7:0] x); return x == 200 ? s27(x) ^ s27(x + 1) : x + e; endfunction
  function automatic logic [7:0] s25(input logic [7:0] x); return x == 200 ? s26(x) ^ s26(x + 1) : x + e; endfunction
  function automatic logic [7:0] s24(input logic [7:0] x); return x == 200 ? s25(x) ^ s25(x + 1) : x + e; endfunction
  function automatic logic [7:0] s23(input logic [7:0] x); return x == 200 ? s24(x) ^ s24(x + 1) : x + e; endfunction
  function automatic logic [7:0] s22(input logic [7:0] x); return x == 200 ? s23(x) ^ s23(x + 1) : x + e; endfunction
  function automatic logic [7:0] s21(input logic [7:0] x); return x == 200 ? s22(x) ^ s22(x + 1) : x + e; endfunction
  function automatic logic [7:0] s20(input logic [7:0] x); return x == 200 ? s21(x) ^ s21(x + 1) : x + e; endfunction
  function automatic logic [7:0] s19(input logic [7:0] x); return x == 200 ? s20(x) ^ s20(x + 1) : x + e; endfunction
  function automatic logic [7:0] s18(input logic [7:0] x); return x == 200 ? s19(x) ^ s19(x + 1) : x + e; endfunction
  function automatic logic [7:0] s17(input logic [7:0] x); return x == 200 ? s18(x) ^ s18(x + 1) : x + e; endfunction
  function automatic logic [7:0] s16(input logic [7:0] x); return x == 200 ? s17(x) ^ s17(x + 1) : x + e; endfunction
  function automatic logic [7:0] s15(input logic [7:0] x); return x == 200 ? s16(x) ^ s16(x + 1) : x + e; endfunction
  function automatic logic [7:0] s14(input logic [7:0] x); return x == 200 ? s15(x) ^ s15(x + 1) : x + e; endfunction
  function automatic logic [7:0] s13(input logic [7:0] x); return x == 200 ? s14(x) ^ s14(x + 1) : x + e; endfunction
  function automatic logic [7:0] s12(input logic [7:0] x); return x == 200 ? s13(x) ^ s13(x + 1) : x + e; endfunction
  function automatic logic [7:0] s11(input logic [7:0] x); return x == 200 ? s12(x) ^ s12(x + 1) : x + e; endfunction
  function automatic logic [7:0] s10(input logic [7:0] x); return x == 200 ? s11(x) ^ s11(x + 1) : x + e; endfunction
  function automatic logic [7:0] s9(input logic [7:0] x); return x == 200 ? s10(x) ^ s10(x + 1) : x + e; endfunction
  function automatic logic [7:0] s8(input logic [7:0] x); return x == 200 ? s9(x) ^ s9(x + 1) : x + e; endfunction
  function automatic logic [7:0] s7(input logic [7:0] x); return x == 200 ? s8(x) ^ s8(x + 1) : x + e; endfunction
  function automatic logic [7:0] s6(input logic [7:0] x); return x == 200 ? s7(x) ^ s7(x + 1) : x + e; endfunction
  function automatic logic [7:0] s5(input logic [7:0] x); return x == 200 ? s6(x) ^ s6(x + 1) : x + e; endfunction
  function automatic logic [7:0] s4(input logic [7:0] x); return x == 200 ? s5(x) ^ s5(x + 1) : x + e; endfunction
  function automatic logic [7:0] s3(input logic [7:0] x); return x == 200 ? s4(x) ^ s4(x + 1) : x + e; endfunction
  function automatic logic [7:0] s2(input logic [7:0] x); return x == 200 ? s3(x) ^ s3(x + 1) : x + e; endfunction
  function automatic logic [7:0] s1(input logic [7:0] x); return x == 200 ? s2(x) ^ s2(x + 1) : x + e; endfunction
  function automatic logic [7:0] s0(input logic [7:0] x); return x == 200 ? s1(x) ^ s1(x + 1) : x + e; endfunction

  // The last one calls the first: recursion over the same paths.
  function automatic logic [7:0] k30(input logic [7:0] x); return x == 0 ? e : k0(x - 1); endfunction
  function automatic logic [7:0] k29(input logic [7:0] x); return x == 200 ? k30(x) ^ k30(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k28(input logic [7:0] x); return x == 200 ? k29(x) ^ k29(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k27(input logic [7:0] x); return x == 200 ? k28(x) ^ k28(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k26(input logic [7:0] x); return x == 200 ? k27(x) ^ k27(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k25(input logic [7:0] x); return x == 200 ? k26(x) ^ k26(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k24(input logic [7:0] x); return x == 200 ? k25(x) ^ k25(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k23(input logic [7:0] x); return x == 200 ? k24(x) ^ k24(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k22(input logic [7:0] x); return x == 200 ? k23(x) ^ k23(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k21(input logic [7:0] x); return x == 200 ? k22(x) ^ k22(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k20(input logic [7:0] x); return x == 200 ? k21(x) ^ k21(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k19(input logic [7:0] x); return x == 200 ? k20(x) ^ k20(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k18(input logic [7:0] x); return x == 200 ? k19(x) ^ k19(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k17(input logic [7:0] x); return x == 200 ? k18(x) ^ k18(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k16(input logic [7:0] x); return x == 200 ? k17(x) ^ k17(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k15(input logic [7:0] x); return x == 200 ? k16(x) ^ k16(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k14(input logic [7:0] x); return x == 200 ? k15(x) ^ k15(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k13(input logic [7:0] x); return x == 200 ? k14(x) ^ k14(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k12(input logic [7:0] x); return x == 200 ? k13(x) ^ k13(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k11(input logic [7:0] x); return x == 200 ? k12(x) ^ k12(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k10(input logic [7:0] x); return x == 200 ? k11(x) ^ k11(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k9(input logic [7:0] x); return x == 200 ? k10(x) ^ k10(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k8(input logic [7:0] x); return x == 200 ? k9(x) ^ k9(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k7(input logic [7:0] x); return x == 200 ? k8(x) ^ k8(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k6(input logic [7:0] x); return x == 200 ? k7(x) ^ k7(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k5(input logic [7:0] x); return x == 200 ? k6(x) ^ k6(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k4(input logic [7:0] x); return x == 200 ? k5(x) ^ k5(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k3(input logic [7:0] x); return x == 200 ? k4(x) ^ k4(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k2(input logic [7:0] x); return x == 200 ? k3(x) ^ k3(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k1(input logic [7:0] x); return x == 200 ? k2(x) ^ k2(x + 1) : x + e + 1; endfunction
  function automatic logic [7:0] k0(input logic [7:0] x); return x == 200 ? k1(x) ^ k1(x + 1) : x + e + 1; endfunction

  function automatic void w30(ref logic [7:0] x, ref logic [7:0] z); x = z; endfunction
  function automatic void w29(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w30(x, z); w30(z, x); end else x = z + 29; endfunction
  function automatic void w28(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w29(x, z); w29(z, x); end else x = z + 28; endfunction
  function automatic void w27(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w28(x, z); w28(z, x); end else x = z + 27; endfunction
  function automatic void w26(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w27(x, z); w27(z, x); end else x = z + 26; endfunction
  function automatic void w25(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w26(x, z); w26(z, x); end else x = z + 25; endfunction
  function automatic void w24(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w25(x, z); w25(z, x); end else x = z + 24; endfunction
  function automatic void w23(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w24(x, z); w24(z, x); end else x = z + 23; endfunction
  function automatic void w22(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w23(x, z); w23(z, x); end else x = z + 22; endfunction
  function automatic void w21(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w22(x, z); w22(z, x); end else x = z + 21; endfunction
  function automatic void w20(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w21(x, z); w21(z, x); end else x = z + 20; endfunction
  function automatic void w19(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w20(x, z); w20(z, x); end else x = z + 19; endfunction
  function automatic void w18(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w19(x, z); w19(z, x); end else x = z + 18; endfunction
  function automatic void w17(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w18(x, z); w18(z, x); end else x = z + 17; endfunction
  function automatic void w16(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w17(x, z); w17(z, x); end else x = z + 16; endfunction
  function automatic void w15(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w16(x, z); w16(z, x); end else x = z + 15; endfunction
  function automatic void w14(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w15(x, z); w15(z, x); end else x = z + 14; endfunction
  function automatic void w13(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w14(x, z); w14(z, x); end else x = z + 13; endfunction
  function automatic void w12(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w13(x, z); w13(z, x); end else x = z + 12; endfunction
  function automatic void w11(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w12(x, z); w12(z, x); end else x = z + 11; endfunction
  function automatic void w10(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w11(x, z); w11(z, x); end else x = z + 10; endfunction
  function automatic void w9(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w10(x, z); w10(z, x); end else x = z + 9; endfunction
  function automatic void w8(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w9(x, z); w9(z, x); end else x = z + 8; endfunction
  function automatic void w7(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w8(x, z); w8(z, x); end else x = z + 7; endfunction
  function automatic void w6(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w7(x, z); w7(z, x); end else x = z + 6; endfunction
  function automatic void w5(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w6(x, z); w6(z, x); end else x = z + 5; endfunction
  function automatic void w4(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w5(x, z); w5(z, x); end else x = z + 4; endfunction
  function automatic void w3(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w4(x, z); w4(z, x); end else x = z + 3; endfunction
  function automatic void w2(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w3(x, z); w3(z, x); end else x = z + 2; endfunction
  function automatic void w1(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w2(x, z); w2(z, x); end else x = z + 1; endfunction
  function automatic void w0(ref logic [7:0] x, ref logic [7:0] z); if (z == 200) begin w1(x, z); w1(z, x); end else x = z + 0; endfunction

  function automatic int g30(input c x); return x.v; endfunction
  function automatic int g29(input c x); return x.v == 200 ? g30(x) + g30(x) : x.v + 1; endfunction
  function automatic int g28(input c x); return x.v == 200 ? g29(x) + g29(x) : x.v + 1; endfunction
  function automatic int g27(input c x); return x.v == 200 ? g28(x) + g28(x) : x.v + 1; endfunction
  function automatic int g26(input c x); return x.v == 200 ? g27(x) + g27(x) : x.v + 1; endfunction
  function automatic int g25(input c x); return x.v == 200 ? g26(x) + g26(x) : x.v + 1; endfunction
  function automatic int g24(input c x); return x.v == 200 ? g25(x) + g25(x) : x.v + 1; endfunction
  function automatic int g23(input c x); return x.v == 200 ? g24(x) + g24(x) : x.v + 1; endfunction
  function automatic int g22(input c x); return x.v == 200 ? g23(x) + g23(x) : x.v + 1; endfunction
  function automatic int g21(input c x); return x.v == 200 ? g22(x) + g22(x) : x.v + 1; endfunction
  function automatic int g20(input c x); return x.v == 200 ? g21(x) + g21(x) : x.v + 1; endfunction
  function automatic int g19(input c x); return x.v == 200 ? g20(x) + g20(x) : x.v + 1; endfunction
  function automatic int g18(input c x); return x.v == 200 ? g19(x) + g19(x) : x.v + 1; endfunction
  function automatic int g17(input c x); return x.v == 200 ? g18(x) + g18(x) : x.v + 1; endfunction
  function automatic int g16(input c x); return x.v == 200 ? g17(x) + g17(x) : x.v + 1; endfunction
  function automatic int g15(input c x); return x.v == 200 ? g16(x) + g16(x) : x.v + 1; endfunction
  function automatic int g14(input c x); return x.v == 200 ? g15(x) + g15(x) : x.v + 1; endfunction
  function automatic int g13(input c x); return x.v == 200 ? g14(x) + g14(x) : x.v + 1; endfunction
  function automatic int g12(input c x); return x.v == 200 ? g13(x) + g13(x) : x.v + 1; endfunction
  function automatic int g11(input c x); return x.v == 200 ? g12(x) + g12(x) : x.v + 1; endfunction
  function automatic int g10(input c x); return x.v == 200 ? g11(x) + g11(x) : x.v + 1; endfunction
  function automatic int g9(input c x); return x.v == 200 ? g10(x) + g10(x) : x.v + 1; endfunction
  function automatic int g8(input c x); return x.v == 200 ? g9(x) + g9(x) : x.v + 1; endfunction
  function automatic int g7(input c x); return x.v == 200 ? g8(x) + g8(x) : x.v + 1; endfunction
  function automatic int g6(input c x); return x.v == 200 ? g7(x) + g7(x) : x.v + 1; endfunction
  function automatic int g5(input c x); return x.v == 200 ? g6(x) + g6(x) : x.v + 1; endfunction
  function automatic int g4(input c x); return x.v == 200 ? g5(x) + g5(x) : x.v + 1; endfunction
  function automatic int g3(input c x); return x.v == 200 ? g4(x) + g4(x) : x.v + 1; endfunction
  function automatic int g2(input c x); return x.v == 200 ? g3(x) + g3(x) : x.v + 1; endfunction
  function automatic int g1(input c x); return x.v == 200 ? g2(x) + g2(x) : x.v + 1; endfunction
  function automatic int g0(input c x); return x.v == 200 ? g1(x) + g1(x) : x.v + 1; endfunction

  assign #1 n = s0(d);
  always_comb y = s0(d);
  always @(r) w0(f, r);
  always @(n) $display("%0t n %0d", $time, n);
  always @(y) $display("%0t y %0d", $time, y);
  always @(f) if ($time > 0) $display("%0t f %0d", $time, f);
  always @(s0(d)) if ($time > 0) $display("%0t event %0d", $time, s0(d));
  always @(posedge k0(d)) if ($time > 0) $display("%0t edge %0d", $time, k0(d));
  initial begin wait (s0(d) == 12); $display("%0t wait", $time); end
  initial begin wait (g0(h) == 5); $display("%0t handle", $time); end
  initial $monitor("%0t monitor %0d", $time, s0(d));
  initial begin
    #2 d = 5;
    #2 d = 10;
    #2 r = 20;
    #2 k.v = 4;
    h = k;
    #2 q = 3;
    #2 force q = s0(d);
    #1 $display("%0t force %0d", $time, q);
    d = 6;
    #1 $display("%0t force %0d", $time, q);
    $finish(0);
  end
endmodule
