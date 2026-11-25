// Conditional chains over wide, signed, mixed-width, two-state, real, string,
// enumeration and packed structure operands (IEEE 1800-2009 11.4.11).
`timescale 1ns/1ns
module tb;
  typedef struct packed { logic [3:0] tag; logic [11:0] body; } pk_t;
  typedef enum logic [2:0] { RED, GREEN, BLUE } color_t;
  logic [7:0] s;
  logic [32:0] v33 [9];
  logic [32:0] y33;
  logic [63:0] v64 [9];
  logic [63:0] y64;
  logic [64:0] v65 [9];
  logic [64:0] y65;
  logic [127:0] v128 [9];
  logic [127:0] y128;
  logic [199:0] v200 [9];
  logic [199:0] y200;
  logic signed [7:0] sg [9];
  logic [3:0] un [9];
  logic signed [15:0] ys; logic [15:0] yu; logic signed [31:0] ym;
  bit [15:0] tw [9]; int yi; logic [15:0] ymx;
  real rl [9]; real yr; int yri;
  string st [9]; string yst;
  color_t co [9]; color_t yco;
  pk_t pk [9]; pk_t ypk;
  logic [127:0] yfl; logic [64:0] yf65;
  task automatic eval(string what);
    y33 = s[0] ? v33[0] : s[1] ? v33[1] : s[2] ? v33[2] : s[3] ? v33[3] : s[4] ? v33[4] : s[5] ? v33[5] : s[6] ? v33[6] : s[7] ? v33[7] : v33[8];
    y64 = s[0] ? v64[0] : s[1] ? v64[1] : s[2] ? v64[2] : s[3] ? v64[3] : s[4] ? v64[4] : s[5] ? v64[5] : s[6] ? v64[6] : s[7] ? v64[7] : v64[8];
    y65 = s[0] ? v65[0] : s[1] ? v65[1] : s[2] ? v65[2] : s[3] ? v65[3] : s[4] ? v65[4] : s[5] ? v65[5] : s[6] ? v65[6] : s[7] ? v65[7] : v65[8];
    y128 = s[0] ? v128[0] : s[1] ? v128[1] : s[2] ? v128[2] : s[3] ? v128[3] : s[4] ? v128[4] : s[5] ? v128[5] : s[6] ? v128[6] : s[7] ? v128[7] : v128[8];
    y200 = s[0] ? v200[0] : s[1] ? v200[1] : s[2] ? v200[2] : s[3] ? v200[3] : s[4] ? v200[4] : s[5] ? v200[5] : s[6] ? v200[6] : s[7] ? v200[7] : v200[8];
    ys = s[0] ? sg[0] : s[1] ? sg[1] : s[2] ? sg[2] : s[3] ? sg[3] : s[4] ? sg[4] : s[5] ? sg[5] : s[6] ? sg[6] : s[7] ? sg[7] : sg[8];
    yu = s[0] ? un[0] : s[1] ? sg[1] : s[2] ? un[2] : s[3] ? sg[3] : s[4] ? un[4] : s[5] ? sg[5] : s[6] ? un[6] : s[7] ? sg[7] : sg[8];
    ym = s[0] ? sg[0] : s[1] ? 12'sh801 : s[2] ? un[2] : s[3] ? -4 : s[4] ? sg[4] : s[5] ? 12'sh805 : s[6] ? un[6] : s[7] ? -8 : sg[8];
    yi = s[0] ? tw[0] : s[1] ? tw[1] : s[2] ? tw[2] : s[3] ? tw[3] : s[4] ? tw[4] : s[5] ? tw[5] : s[6] ? tw[6] : s[7] ? tw[7] : tw[8];
    ymx = s[0] ? v33[0][15:0] : s[1] ? tw[1] : s[2] ? v33[2][15:0] : s[3] ? tw[3] : s[4] ? v33[4][15:0] : s[5] ? tw[5] : s[6] ? v33[6][15:0] : s[7] ? tw[7] : tw[8];
    yr = s[0] ? rl[0] : s[1] ? rl[1] : s[2] ? rl[2] : s[3] ? rl[3] : s[4] ? rl[4] : s[5] ? rl[5] : s[6] ? rl[6] : s[7] ? rl[7] : rl[8];
    yri = s[0] ? 40 : s[1] ? rl[1] : s[2] ? 42 : s[3] ? rl[3] : s[4] ? 44 : s[5] ? rl[5] : s[6] ? 46 : s[7] ? rl[7] : 99;
    yst = s[0] ? st[0] : s[1] ? st[1] : s[2] ? st[2] : s[3] ? st[3] : s[4] ? st[4] : s[5] ? st[5] : s[6] ? st[6] : s[7] ? st[7] : st[8];
    yco = s[0] ? co[0] : s[1] ? co[1] : s[2] ? co[2] : s[3] ? co[3] : s[4] ? co[4] : s[5] ? co[5] : s[6] ? co[6] : s[7] ? co[7] : co[8];
    ypk = s[0] ? pk[0] : s[1] ? pk[1] : s[2] ? pk[2] : s[3] ? pk[3] : s[4] ? pk[4] : s[5] ? pk[5] : s[6] ? pk[6] : s[7] ? pk[7] : pk[8];
    yfl = s[0] ? v128[0] : s[1] ? '1 : s[2] ? 'x : s[3] ? {v64[3], v64[4]} : s[4] ? '0 : 'z;
    yf65 = s[0] ? 'z : s[1] ? v65[1] : s[2] ? '1 : {1'b0, v64[5]};
    $display("%s s=%b", what, s);
    $display("  w33=%h w64=%h w65=%h", y33, y64, y65);
    $display("  w128=%h", y128);
    $display("  w200=%h", y200);
    $display("  yfl=%h yf65=%h", yfl, yf65);
    $display("  ys=%0d yu=%h ym=%0d yi=%0d ymx=%b", ys, yu, ym, yi, ymx);
    $display("  yr=%g yri=%0d yst='%s' yco=%0d ypk=%h", yr, yri, yst, yco, ypk);
  endtask
  initial begin
    for (int i = 0; i <= 8; i++) begin
      v33[i] = {1'b1, 32'hdead0000 + 32'(i)};
      v64[i] = 64'h0123456789abcdef ^ {8{8'(i)}};
      v65[i] = {1'(i), 64'hfedcba9876543210} ;
      v128[i] = {4{32'hcafe0000 + 32'(i * 17)}};
      v200[i] = {8'(i + 1), 192'h0}; v200[i][99:68] = 32'h5a5a5a5a; v200[i][7:0] = 8'(i);
      sg[i] = -8'sd3 - 8'(i * 5); un[i] = 4'(i + 9);
      tw[i] = 16'(i * 1000 + 7); rl[i] = i * 1.25 + 0.5;
      st[i] = $sformatf("str%0d", i);
      co[i] = color_t'(i % 3);
      pk[i] = '{tag: 4'(i), body: 12'(i * 273)};
    end
    for (int pos = 0; pos < 8; pos++) begin
      s = '0; s[pos] = 1'b1; eval("one");
      s = '0; s[pos] = 1'bx; eval("x");
      s = '0; s[pos] = 1'bz; if (pos > 0) s[pos - 1] = 1'bx; eval("zx");
      s = '0; s[pos] = 1'bx; s[7] = 1'b1; eval("x1");
    end
    s = '0; eval("none");
    s = 'x; eval("allx");
    // Equal integral and string alternatives survive an ambiguous predicate.
    for (int i = 0; i <= 8; i++) begin
      v33[i] = 33'h1_0000_ffff; v64[i] = '1; v65[i] = 65'h5; v128[i] = 128'h77; v200[i] = 200'h9;
      sg[i] = -8'sd7; un[i] = 4'hf; tw[i] = 16'd77; st[i] = "same"; co[i] = BLUE; pk[i] = 16'habcd;
    end
    s = 'x; eval("same");
    s = 8'b0000x000; st[4] = "other"; v128[3][64] = 1'b0; eval("near");
  end
endmodule
