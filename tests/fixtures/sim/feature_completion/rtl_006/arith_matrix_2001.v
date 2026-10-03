// IEEE 1364-2001 4.1.5-4.1.6, 4.1.12, 4.4-4.5: the arith_matrix.sv operand
// matrix in Verilog-2001 syntax (signed regs, memories, memory bit-selects).
module arith(dummy);
  input dummy;
  parameter W = 1;
  parameter SLOT = 0;
  parameter N = 13;
  localparam [255:0] PAT =
      256'h9e3779b97f4a7c15_f39cc0605cedc834_1082276bf3a27251_f86c6a11d0c18e95;
  reg [W-1:0] v [0:N-1];
  reg [W-1:0] ua, ub;
  reg signed [W-1:0] sa, sb;
  integer i, j;

  initial begin
    v[0] = 0;
    v[1] = 1;
    v[2] = v[1] + v[1];
    v[3] = v[2] + v[1];
    v[4] = {W{1'b1}};
    v[5] = v[1] << (W - 1);
    v[6] = v[5] - v[1];
    v[7] = v[5] + v[1];
    v[8] = PAT[W-1:0];
    v[9] = ~v[8];
    v[10] = v[4] - v[2];
    v[11] = {W{1'bx}};
    v[12] = v[8];
    v[12][0] = 1'bz;
    #(SLOT);
    for (i = 0; i < N; i = i + 1) begin
      ua = v[i];
      sa = v[i];
      $display("%0d %0d n %h %h", W, i, -sa, -ua);
      for (j = 0; j < N; j = j + 1) begin
        ub = v[j];
        sb = v[j];
        $display("%0d %0d %0d s %h %h %h %h %h %h u %h %h %h %h %h %h m %h %h %h %h",
                 W, i, j,
                 sa + sb, sa - sb, sa * sb, sa / sb, sa % sb, sa ** sb,
                 ua + ub, ua - ub, ua * ub, ua / ub, ua % ub, ua ** ub,
                 sa / ub, sa % ub, sa ** ub, ua ** sb);
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
  wire d = 1'b0;
  arith #(1, 1) w1(d);
  arith #(31, 2) w31(d);
  arith #(32, 3) w32(d);
  arith #(63, 4) w63(d);
  arith #(64, 5) w64(d);
  arith #(65, 6) w65(d);
  arith #(129, 7) w129(d);
  initial #8 $finish(0);
endmodule
