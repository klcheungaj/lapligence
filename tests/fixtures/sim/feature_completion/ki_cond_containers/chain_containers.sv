// Conditional chains over queues, dynamic arrays and associative arrays: 4 to 32 levels, every predicate position (IEEE 1800-2009 11.4.11).

module tb;
  logic [31:0] s;
  logic [7:0] q0[$] = '{8'h5a};
  logic [7:0] q1[$] = '{8'h5a, 8'ha5};
  logic [7:0] q2[$] = '{8'ha5, 8'h5a, 8'ha5};
  logic [7:0] q3[$] = '{8'ha5};
  logic [7:0] q4[$] = '{8'h5a, 8'ha5};
  logic [7:0] q5[$] = '{8'h5a, 8'ha5, 8'h5a};
  logic [7:0] q6[$] = '{8'ha5};
  logic [7:0] q7[$] = '{8'ha5, 8'h5a};
  logic [7:0] q8[$] = '{8'h5a, 8'ha5, 8'h5a};
  logic [7:0] q9[$] = '{8'h5a};
  logic [7:0] q10[$] = '{8'ha5, 8'h5a};
  logic [7:0] q11[$] = '{8'ha5, 8'h5a, 8'ha5};
  logic [7:0] q12[$] = '{8'h5a};
  logic [7:0] q13[$] = '{8'h5a, 8'ha5};
  logic [7:0] q14[$] = '{8'ha5, 8'h5a, 8'ha5};
  logic [7:0] q15[$] = '{8'ha5};
  logic [7:0] q16[$] = '{8'h5a, 8'ha5};
  logic [7:0] q17[$] = '{8'h5a, 8'ha5, 8'h5a};
  logic [7:0] q18[$] = '{8'ha5};
  logic [7:0] q19[$] = '{8'ha5, 8'h5a};
  logic [7:0] q20[$] = '{8'h5a, 8'ha5, 8'h5a};
  logic [7:0] q21[$] = '{8'h5a};
  logic [7:0] q22[$] = '{8'ha5, 8'h5a};
  logic [7:0] q23[$] = '{8'ha5, 8'h5a, 8'ha5};
  logic [7:0] q24[$] = '{8'h5a};
  logic [7:0] q25[$] = '{8'h5a, 8'ha5};
  logic [7:0] q26[$] = '{8'ha5, 8'h5a, 8'ha5};
  logic [7:0] q27[$] = '{8'ha5};
  logic [7:0] q28[$] = '{8'h5a, 8'ha5};
  logic [7:0] q29[$] = '{8'h5a, 8'ha5, 8'h5a};
  logic [7:0] q30[$] = '{8'ha5};
  logic [7:0] q31[$] = '{8'ha5, 8'h5a};
  logic [7:0] q32[$] = '{8'h5a, 8'ha5, 8'h5a};
  logic [7:0] d0[] = '{8'h5a};
  logic [7:0] d1[] = '{8'h5a, 8'ha5};
  logic [7:0] d2[] = '{8'ha5, 8'h5a, 8'ha5};
  logic [7:0] d3[] = '{8'ha5};
  logic [7:0] d4[] = '{8'h5a, 8'ha5};
  logic [7:0] d5[] = '{8'h5a, 8'ha5, 8'h5a};
  logic [7:0] d6[] = '{8'ha5};
  logic [7:0] d7[] = '{8'ha5, 8'h5a};
  logic [7:0] d8[] = '{8'h5a, 8'ha5, 8'h5a};
  logic [7:0] d9[] = '{8'h5a};
  logic [7:0] d10[] = '{8'ha5, 8'h5a};
  logic [7:0] d11[] = '{8'ha5, 8'h5a, 8'ha5};
  logic [7:0] d12[] = '{8'h5a};
  logic [7:0] d13[] = '{8'h5a, 8'ha5};
  logic [7:0] d14[] = '{8'ha5, 8'h5a, 8'ha5};
  logic [7:0] d15[] = '{8'ha5};
  logic [7:0] d16[] = '{8'h5a, 8'ha5};
  logic [7:0] d17[] = '{8'h5a, 8'ha5, 8'h5a};
  logic [7:0] d18[] = '{8'ha5};
  logic [7:0] d19[] = '{8'ha5, 8'h5a};
  logic [7:0] d20[] = '{8'h5a, 8'ha5, 8'h5a};
  logic [7:0] d21[] = '{8'h5a};
  logic [7:0] d22[] = '{8'ha5, 8'h5a};
  logic [7:0] d23[] = '{8'ha5, 8'h5a, 8'ha5};
  logic [7:0] d24[] = '{8'h5a};
  logic [7:0] d25[] = '{8'h5a, 8'ha5};
  logic [7:0] d26[] = '{8'ha5, 8'h5a, 8'ha5};
  logic [7:0] d27[] = '{8'ha5};
  logic [7:0] d28[] = '{8'h5a, 8'ha5};
  logic [7:0] d29[] = '{8'h5a, 8'ha5, 8'h5a};
  logic [7:0] d30[] = '{8'ha5};
  logic [7:0] d31[] = '{8'ha5, 8'h5a};
  logic [7:0] d32[] = '{8'h5a, 8'ha5, 8'h5a};
  logic [7:0] m0[string] = '{"a": 8'h5a, "c": 8'ha5};
  logic [7:0] m1[string] = '{"a": 8'h5a, "b": 8'ha5};
  logic [7:0] m2[string] = '{"a": 8'ha5};
  logic [7:0] m3[string] = '{"a": 8'ha5, "b": 8'h5a, "c": 8'ha5};
  logic [7:0] m4[string] = '{"a": 8'h5a};
  logic [7:0] m5[string] = '{"a": 8'h5a, "b": 8'ha5};
  logic [7:0] m6[string] = '{"a": 8'ha5, "c": 8'h5a};
  logic [7:0] m7[string] = '{"a": 8'ha5, "b": 8'h5a};
  logic [7:0] m8[string] = '{"a": 8'h5a};
  logic [7:0] m9[string] = '{"a": 8'h5a, "b": 8'ha5, "c": 8'h5a};
  logic [7:0] m10[string] = '{"a": 8'ha5};
  logic [7:0] m11[string] = '{"a": 8'ha5, "b": 8'h5a};
  logic [7:0] m12[string] = '{"a": 8'h5a, "c": 8'ha5};
  logic [7:0] m13[string] = '{"a": 8'h5a, "b": 8'ha5};
  logic [7:0] m14[string] = '{"a": 8'ha5};
  logic [7:0] m15[string] = '{"a": 8'ha5, "b": 8'h5a, "c": 8'ha5};
  logic [7:0] m16[string] = '{"a": 8'h5a};
  logic [7:0] m17[string] = '{"a": 8'h5a, "b": 8'ha5};
  logic [7:0] m18[string] = '{"a": 8'ha5, "c": 8'h5a};
  logic [7:0] m19[string] = '{"a": 8'ha5, "b": 8'h5a};
  logic [7:0] m20[string] = '{"a": 8'h5a};
  logic [7:0] m21[string] = '{"a": 8'h5a, "b": 8'ha5, "c": 8'h5a};
  logic [7:0] m22[string] = '{"a": 8'ha5};
  logic [7:0] m23[string] = '{"a": 8'ha5, "b": 8'h5a};
  logic [7:0] m24[string] = '{"a": 8'h5a, "c": 8'ha5};
  logic [7:0] m25[string] = '{"a": 8'h5a, "b": 8'ha5};
  logic [7:0] m26[string] = '{"a": 8'ha5};
  logic [7:0] m27[string] = '{"a": 8'ha5, "b": 8'h5a, "c": 8'ha5};
  logic [7:0] m28[string] = '{"a": 8'h5a};
  logic [7:0] m29[string] = '{"a": 8'h5a, "b": 8'ha5};
  logic [7:0] m30[string] = '{"a": 8'ha5, "c": 8'h5a};
  logic [7:0] m31[string] = '{"a": 8'ha5, "b": 8'h5a};
  logic [7:0] m32[string] = '{"a": 8'h5a};
  logic [7:0] k0[int] = '{1: 8'h5a, 30: 8'ha5};
  logic [7:0] k1[int] = '{1: 8'h5a, -2: 8'ha5};
  logic [7:0] k2[int] = '{1: 8'ha5};
  logic [7:0] k3[int] = '{1: 8'ha5, -2: 8'h5a, 30: 8'ha5};
  logic [7:0] k4[int] = '{1: 8'h5a};
  logic [7:0] k5[int] = '{1: 8'h5a, -2: 8'ha5};
  logic [7:0] k6[int] = '{1: 8'ha5, 30: 8'h5a};
  logic [7:0] k7[int] = '{1: 8'ha5, -2: 8'h5a};
  logic [7:0] k8[int] = '{1: 8'h5a};
  logic [7:0] b0[$:1] = '{8'h5a};
  logic [7:0] b1[$:1] = '{8'h5a, 8'ha5};
  logic [7:0] b2[$:1] = '{8'ha5};
  logic [7:0] b3[$:1] = '{8'ha5, 8'h5a};
  logic [7:0] b4[$:1] = '{8'h5a};
  logic [7:0] b5[$:1] = '{8'h5a, 8'ha5};
  logic [7:0] b6[$:1] = '{8'ha5};
  logic [7:0] b7[$:1] = '{8'ha5, 8'h5a};
  logic [7:0] b8[$:1] = '{8'h5a};
  logic [199:0] w0[$] = '{200'h5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a};
  logic [199:0] w1[$] = '{200'h5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a, 200'ha5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5};
  logic [199:0] w2[$] = '{200'ha5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5, 200'h5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a, 200'ha5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5};
  logic [199:0] w3[$] = '{200'ha5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5};
  logic [199:0] w4[$] = '{200'h5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a, 200'ha5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5};
  logic [199:0] w5[$] = '{200'h5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a, 200'ha5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5, 200'h5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a};
  logic [199:0] w6[$] = '{200'ha5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5};
  logic [199:0] w7[$] = '{200'ha5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5, 200'h5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a};
  logic [199:0] w8[$] = '{200'h5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a, 200'ha5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5, 200'h5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a};
  int u0[$] = '{32'h5a5a5a5a};
  int u1[$] = '{32'h5a5a5a5a, 32'ha5a5a5a5};
  int u2[$] = '{32'ha5a5a5a5, 32'h5a5a5a5a, 32'ha5a5a5a5};
  int u3[$] = '{32'ha5a5a5a5};
  int u4[$] = '{32'h5a5a5a5a, 32'ha5a5a5a5};
  int u5[$] = '{32'h5a5a5a5a, 32'ha5a5a5a5, 32'h5a5a5a5a};
  int u6[$] = '{32'ha5a5a5a5};
  int u7[$] = '{32'ha5a5a5a5, 32'h5a5a5a5a};
  int u8[$] = '{32'h5a5a5a5a, 32'ha5a5a5a5, 32'h5a5a5a5a};
  logic [7:0] yq[$];
  logic [7:0] yd[];
  logic [7:0] ym[string];
  logic [7:0] yk[int];
  logic [7:0] yb[$:1];
  logic [199:0] yw[$];
  int yu[$];
  string text;
  task automatic check(input string tag, input int n, i);
    case (n)
      4: begin
yq = s[0] ? q0 : s[1] ? q1 : s[2] ? q2 : s[3] ? q3 : q4;
yd = s[0] ? d0 : s[1] ? d1 : s[2] ? d2 : s[3] ? d3 : d4;
ym = s[0] ? m0 : s[1] ? m1 : s[2] ? m2 : s[3] ? m3 : m4;
yk = s[0] ? k0 : s[1] ? k1 : s[2] ? k2 : s[3] ? k3 : k4;
yb = s[0] ? b0 : s[1] ? b1 : s[2] ? b2 : s[3] ? b3 : b4;
yw = s[0] ? w0 : s[1] ? w1 : s[2] ? w2 : s[3] ? w3 : w4;
yu = s[0] ? u0 : s[1] ? u1 : s[2] ? u2 : s[3] ? u3 : u4;
      end
      8: begin
yq = s[0] ? q0 : s[1] ? q1 : s[2] ? q2 : s[3] ? q3 : s[4] ? q4 : s[5] ? q5 : s[6] ? q6 : s[7] ? q7 :
        q8;
yd = s[0] ? d0 : s[1] ? d1 : s[2] ? d2 : s[3] ? d3 : s[4] ? d4 : s[5] ? d5 : s[6] ? d6 : s[7] ? d7 :
        d8;
ym = s[0] ? m0 : s[1] ? m1 : s[2] ? m2 : s[3] ? m3 : s[4] ? m4 : s[5] ? m5 : s[6] ? m6 : s[7] ? m7 :
        m8;
yk = s[0] ? k0 : s[1] ? k1 : s[2] ? k2 : s[3] ? k3 : s[4] ? k4 : s[5] ? k5 : s[6] ? k6 : s[7] ? k7 :
        k8;
yb = s[0] ? b0 : s[1] ? b1 : s[2] ? b2 : s[3] ? b3 : s[4] ? b4 : s[5] ? b5 : s[6] ? b6 : s[7] ? b7 :
        b8;
yw = s[0] ? w0 : s[1] ? w1 : s[2] ? w2 : s[3] ? w3 : s[4] ? w4 : s[5] ? w5 : s[6] ? w6 : s[7] ? w7 :
        w8;
yu = s[0] ? u0 : s[1] ? u1 : s[2] ? u2 : s[3] ? u3 : s[4] ? u4 : s[5] ? u5 : s[6] ? u6 : s[7] ? u7 :
        u8;
      end
      16: begin
yq = s[0] ? q0 : s[1] ? q1 : s[2] ? q2 : s[3] ? q3 : s[4] ? q4 : s[5] ? q5 : s[6] ? q6 : s[7] ? q7 :
        s[8] ? q8 : s[9] ? q9 : s[10] ? q10 : s[11] ? q11 : s[12] ? q12 : s[13] ? q13 : s[14] ? q14
        : s[15] ? q15 : q16;
yd = s[0] ? d0 : s[1] ? d1 : s[2] ? d2 : s[3] ? d3 : s[4] ? d4 : s[5] ? d5 : s[6] ? d6 : s[7] ? d7 :
        s[8] ? d8 : s[9] ? d9 : s[10] ? d10 : s[11] ? d11 : s[12] ? d12 : s[13] ? d13 : s[14] ? d14
        : s[15] ? d15 : d16;
ym = s[0] ? m0 : s[1] ? m1 : s[2] ? m2 : s[3] ? m3 : s[4] ? m4 : s[5] ? m5 : s[6] ? m6 : s[7] ? m7 :
        s[8] ? m8 : s[9] ? m9 : s[10] ? m10 : s[11] ? m11 : s[12] ? m12 : s[13] ? m13 : s[14] ? m14
        : s[15] ? m15 : m16;
      end
      32: begin
yq = s[0] ? q0 : s[1] ? q1 : s[2] ? q2 : s[3] ? q3 : s[4] ? q4 : s[5] ? q5 : s[6] ? q6 : s[7] ? q7 :
        s[8] ? q8 : s[9] ? q9 : s[10] ? q10 : s[11] ? q11 : s[12] ? q12 : s[13] ? q13 : s[14] ? q14
        : s[15] ? q15 : s[16] ? q16 : s[17] ? q17 : s[18] ? q18 : s[19] ? q19 : s[20] ? q20 : s[21]
        ? q21 : s[22] ? q22 : s[23] ? q23 : s[24] ? q24 : s[25] ? q25 : s[26] ? q26 : s[27] ? q27 :
        s[28] ? q28 : s[29] ? q29 : s[30] ? q30 : s[31] ? q31 : q32;
yd = s[0] ? d0 : s[1] ? d1 : s[2] ? d2 : s[3] ? d3 : s[4] ? d4 : s[5] ? d5 : s[6] ? d6 : s[7] ? d7 :
        s[8] ? d8 : s[9] ? d9 : s[10] ? d10 : s[11] ? d11 : s[12] ? d12 : s[13] ? d13 : s[14] ? d14
        : s[15] ? d15 : s[16] ? d16 : s[17] ? d17 : s[18] ? d18 : s[19] ? d19 : s[20] ? d20 : s[21]
        ? d21 : s[22] ? d22 : s[23] ? d23 : s[24] ? d24 : s[25] ? d25 : s[26] ? d26 : s[27] ? d27 :
        s[28] ? d28 : s[29] ? d29 : s[30] ? d30 : s[31] ? d31 : d32;
ym = s[0] ? m0 : s[1] ? m1 : s[2] ? m2 : s[3] ? m3 : s[4] ? m4 : s[5] ? m5 : s[6] ? m6 : s[7] ? m7 :
        s[8] ? m8 : s[9] ? m9 : s[10] ? m10 : s[11] ? m11 : s[12] ? m12 : s[13] ? m13 : s[14] ? m14
        : s[15] ? m15 : s[16] ? m16 : s[17] ? m17 : s[18] ? m18 : s[19] ? m19 : s[20] ? m20 : s[21]
        ? m21 : s[22] ? m22 : s[23] ? m23 : s[24] ? m24 : s[25] ? m25 : s[26] ? m26 : s[27] ? m27 :
        s[28] ? m28 : s[29] ? m29 : s[30] ? m30 : s[31] ? m31 : m32;
      end
    endcase
    begin
      $write("q n=%0d %s %0d:", n, tag, i);
      foreach (yq[j]) $write(" %h", yq[j]);
      $write("\n");
    end
    begin
      $write("d n=%0d %s %0d:", n, tag, i);
      foreach (yd[j]) $write(" %h", yd[j]);
      $write("\n");
    end
    begin
      $write("m n=%0d %s %0d:", n, tag, i);
      foreach (ym[j]) $write(" %s:%h", j, ym[j]);
      $write("\n");
    end
    if (n <= 8) begin
      $write("k n=%0d %s %0d:", n, tag, i);
      foreach (yk[j]) $write(" %0d:%h", j, yk[j]);
      $write("\n");
    end
    if (n <= 8) begin
      $write("b n=%0d %s %0d:", n, tag, i);
      foreach (yb[j]) $write(" %h", yb[j]);
      $write("\n");
    end
    if (n <= 8) begin
      $write("w n=%0d %s %0d:", n, tag, i);
      foreach (yw[j]) $write(" %h", yw[j]);
      $write("\n");
    end
    if (n <= 8) begin
      $write("u n=%0d %s %0d:", n, tag, i);
      foreach (yu[j]) $write(" %h", yu[j]);
      $write("\n");
    end
  endtask
  // Predicates: one of them 1, X or Z; an X followed by a 1; an X and a Z two further on; none; all X.
  task automatic drive(input int n);
    for (int i = 0; i < n; i++) begin
      s = '0; s[i] = 1'b1; check("1", n, i);
      s[i] = 1'bx; check("x", n, i);
      s[i] = 1'bz; check("z", n, i);
      if (i + 1 < n) begin s[i] = 1'bx; s[i + 1] = 1'b1; check("x1", n, i); s[i + 1] = 1'b0; end
      if (i + 2 < n) begin s[i] = 1'bx; s[i + 2] = 1'bz; check("xz", n, i); end
    end
    s = '0; check("none", n, 0);
    s = 'x; check("allx", n, 0);
  endtask
  initial begin drive(4); drive(8); drive(16); drive(32); end
endmodule
