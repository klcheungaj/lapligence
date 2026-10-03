// V2001 4.1.5-4.1.6, 4.4-4.5; SV2009 11.4.3, 11.4.10, 11.6-11.8, Table 11-4.
// Runtime operand matrix at widths 1/31/32/63/64/65/127/128/129. Operands are
// derived from W so the oracle can rebuild them: 0, 1, 2, 3, all ones, signed
// minimum, signed maximum, minimum+1, a fixed bit pattern, its complement,
// -3, all X, and the pattern with a Z in bit 0.
module arith #(parameter int W = 1, parameter int SLOT = 0);
  localparam int N = 13;
  localparam logic [255:0] PAT =
      256'h9e3779b97f4a7c15_f39cc0605cedc834_1082276bf3a27251_f86c6a11d0c18e95;
  logic [W-1:0] v [0:N-1];
  logic [W-1:0] ua, ub;
  logic signed [W-1:0] sa, sb;

  initial begin
    v[0] = 0;
    v[1] = 1;
    v[2] = 2;
    v[3] = 3;
    v[4] = '1;
    v[5] = 1;
    v[5] = v[5] << (W - 1);
    v[6] = v[5] - 1;
    v[7] = v[5] + 1;
    v[8] = PAT[W-1:0];
    v[9] = ~v[8];
    v[10] = v[4] - 2;
    v[11] = 'x;
    v[12] = v[8];
    v[12][0] = 1'bz;
    #(SLOT);
    for (int i = 0; i < N; i++) begin
      ua = v[i];
      sa = v[i];
      $display("%0d %0d n %h %h", W, i, -sa, -ua);
      for (int j = 0; j < N; j++) begin
        ub = v[j];
        sb = v[j];
        $display("%0d %0d %0d s %h %h %h %h %h %h u %h %h %h %h %h %h m %h %h %h %h",
                 W, i, j,
                 sa + sb, sa - sb, sa * sb, sa / sb, sa % sb, sa ** sb,
                 ua + ub, ua - ub, ua * ub, ua / ub, ua % ub, ua ** ub,
                 sa / ub, sa % ub, sa ** ub, ua ** sb);
        // Shifting a partly unknown value keeps known bits beside unknown
        // ones, so those rows print bits rather than mixed hex digits.
        if (i < 11)
          $display("%0d %0d %0d sh %h %h %h %h %h %h", W, i, j,
                   ua << ub, ua >> ub, sa >>> ub, sa <<< ub, sa >>> sb, ua >>> sb);
        else
          $display("%0d %0d %0d sh %b %b %b %b %b %b", W, i, j,
                   ua << ub, ua >> ub, sa >>> ub, sa <<< ub, sa >>> sb, ua >>> sb);
      end
    end
  end
endmodule

module tb;
  arith #(.W(1), .SLOT(1)) w1();
  arith #(.W(31), .SLOT(2)) w31();
  arith #(.W(32), .SLOT(3)) w32();
  arith #(.W(63), .SLOT(4)) w63();
  arith #(.W(64), .SLOT(5)) w64();
  arith #(.W(65), .SLOT(6)) w65();
  arith #(.W(127), .SLOT(7)) w127();
  arith #(.W(128), .SLOT(8)) w128();
  arith #(.W(129), .SLOT(9)) w129();
  initial #10 $finish(0);
endmodule
