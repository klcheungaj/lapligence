// V2001 4.4-4.5; SV2009 11.6-11.8: mixed widths and signs in an assignment
// context. Operands of WA and WB bits (formulas as in arith_matrix.sv) are
// extended to max(WA, WB, WR) as signed only when both operands are signed;
// the power operator sizes and signs from its base alone and takes a
// self-determined exponent; shifts extend the left operand only.
module mixed #(parameter int WA = 1, parameter int WB = 1, parameter int WR = 1,
               parameter int SLOT = 0);
  localparam int N = 13;
  localparam logic [255:0] PAT =
      256'h9e3779b97f4a7c15_f39cc0605cedc834_1082276bf3a27251_f86c6a11d0c18e95;
  logic [WA-1:0] va [0:N-1];
  logic [WB-1:0] vb [0:N-1];
  logic [WA-1:0] ua;
  logic signed [WA-1:0] sa;
  logic [WB-1:0] ub;
  logic signed [WB-1:0] sb;
  logic [WR-1:0] r [0:13];

  initial begin
    va[0] = '0; va[1] = '0; va[1][0] = 1'b1; va[2] = va[1] + va[1]; va[3] = va[2] + va[1];
    va[4] = '1; va[5] = va[1] << (WA - 1); va[6] = va[5] - va[1]; va[7] = va[5] + va[1];
    va[8] = {((WA + 255) / 256){PAT}}; va[9] = ~va[8]; va[10] = va[4] - va[2]; va[11] = 'x;
    va[12] = va[8]; va[12][0] = 1'bz;
    vb[0] = '0; vb[1] = '0; vb[1][0] = 1'b1; vb[2] = vb[1] + vb[1]; vb[3] = vb[2] + vb[1];
    vb[4] = '1; vb[5] = vb[1] << (WB - 1); vb[6] = vb[5] - vb[1]; vb[7] = vb[5] + vb[1];
    vb[8] = {((WB + 255) / 256){PAT}}; vb[9] = ~vb[8]; vb[10] = vb[4] - vb[2]; vb[11] = 'x;
    vb[12] = vb[8]; vb[12][0] = 1'bz;
    #(SLOT);
    for (int i = 0; i < N; i++) begin
      ua = va[i];
      sa = va[i];
      for (int j = 0; j < N; j++) begin
        ub = vb[j];
        sb = vb[j];
        r[0] = sa + sb;
        r[1] = sa - sb;
        r[2] = sa * sb;
        r[3] = sa / sb;
        r[4] = sa % sb;
        r[5] = sa + ub;
        r[6] = sa * ub;
        r[7] = sa / ub;
        r[8] = sa % ub;
        r[9] = ua - sb;
        r[10] = sa ** sb;
        r[11] = ua ** sb;
        r[12] = sa >>> ub;
        r[13] = ua << sb;
        $display("%0d %0d %0d %0d %0d x %h %h %h %h %h %h %h %h %h %h p %h %h", WA, WB, WR,
                 i, j, r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7], r[8], r[9],
                 r[10], r[11]);
        if (i < 11)
          $display("%0d %0d %0d %0d %0d sh %h %h", WA, WB, WR, i, j, r[12], r[13]);
      end
    end
  end
endmodule

module tb;
  mixed #(.WA(31), .WB(65), .WR(129), .SLOT(1)) m1();
  mixed #(.WA(64), .WB(63), .WR(129), .SLOT(2)) m2();
  mixed #(.WA(129), .WB(32), .WR(64), .SLOT(3)) m3();
  mixed #(.WA(65), .WB(1), .WR(33), .SLOT(4)) m4();
  mixed #(.WA(32), .WB(32), .WR(65), .SLOT(5)) m5();
  mixed #(.WA(1), .WB(129), .WR(127), .SLOT(6)) m6();
  initial #7 $finish(0);
endmodule
