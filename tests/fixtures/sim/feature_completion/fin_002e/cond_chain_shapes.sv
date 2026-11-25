// Conditional operators nested in the first alternative, in both, in the
// predicate, and with alternatives of different types along one chain
// (IEEE 1800-2009 11.4.11, 11.6.1).
`timescale 1ns/1ns
module tb;
  logic [3:0] s4;
  logic [15:0] d4 [5];
  logic [15:0] l4;
  task automatic calc4();
    l4 = (s4[3] ? (s4[2] ? (s4[1] ? (s4[0] ? d4[4] : d4[0]) : d4[1]) : d4[2]) : d4[3]);
  endtask
  logic [7:0] s8;
  logic [15:0] d8 [9];
  logic [15:0] l8;
  task automatic calc8();
    l8 = (s8[7] ? (s8[6] ? (s8[5] ? (s8[4] ? (s8[3] ? (s8[2] ? (s8[1] ? (s8[0] ? d8[8] : d8[0]) : d8[1]) : d8[2]) : d8[3]) : d8[4]) : d8[5]) : d8[6]) : d8[7]);
  endtask
  logic [15:0] s16;
  logic [15:0] d16 [17];
  logic [15:0] l16;
  task automatic calc16();
    l16 = (s16[15] ? (s16[14] ? (s16[13] ? (s16[12] ? (s16[11] ? (s16[10] ? (s16[9] ? (s16[8] ? (s16[7] ? (s16[6] ? (s16[5] ? (s16[4] ? (s16[3] ? (s16[2] ? (s16[1] ? (s16[0] ? d16[16] : d16[0]) : d16[1]) : d16[2]) : d16[3]) : d16[4]) : d16[5]) : d16[6]) : d16[7]) : d16[8]) : d16[9]) : d16[10]) : d16[11]) : d16[12]) : d16[13]) : d16[14]) : d16[15]);
  endtask
  logic [31:0] s32;
  logic [15:0] d32 [33];
  logic [15:0] l32;
  task automatic calc32();
    l32 = (s32[31] ? (s32[30] ? (s32[29] ? (s32[28] ? (s32[27] ? (s32[26] ? (s32[25] ? (s32[24] ? (s32[23] ? (s32[22] ? (s32[21] ? (s32[20] ? (s32[19] ? (s32[18] ? (s32[17] ? (s32[16] ? (s32[15] ? (s32[14] ? (s32[13] ? (s32[12] ? (s32[11] ? (s32[10] ? (s32[9] ? (s32[8] ? (s32[7] ? (s32[6] ? (s32[5] ? (s32[4] ? (s32[3] ? (s32[2] ? (s32[1] ? (s32[0] ? d32[32] : d32[0]) : d32[1]) : d32[2]) : d32[3]) : d32[4]) : d32[5]) : d32[6]) : d32[7]) : d32[8]) : d32[9]) : d32[10]) : d32[11]) : d32[12]) : d32[13]) : d32[14]) : d32[15]) : d32[16]) : d32[17]) : d32[18]) : d32[19]) : d32[20]) : d32[21]) : d32[22]) : d32[23]) : d32[24]) : d32[25]) : d32[26]) : d32[27]) : d32[28]) : d32[29]) : d32[30]) : d32[31]);
  endtask
  logic [4:0] t; logic [15:0] leaf [32]; logic [15:0] yt;
  task automatic calct();
    yt = (t[4] ? (t[3] ? (t[2] ? (t[1] ? (t[0] ? leaf[31] : leaf[30]) : (t[0] ? leaf[29] : leaf[28])) : (t[1] ? (t[0] ? leaf[27] : leaf[26]) : (t[0] ? leaf[25] : leaf[24]))) : (t[2] ? (t[1] ? (t[0] ? leaf[23] : leaf[22]) : (t[0] ? leaf[21] : leaf[20])) : (t[1] ? (t[0] ? leaf[19] : leaf[18]) : (t[0] ? leaf[17] : leaf[16])))) : (t[3] ? (t[2] ? (t[1] ? (t[0] ? leaf[15] : leaf[14]) : (t[0] ? leaf[13] : leaf[12])) : (t[1] ? (t[0] ? leaf[11] : leaf[10]) : (t[0] ? leaf[9] : leaf[8]))) : (t[2] ? (t[1] ? (t[0] ? leaf[7] : leaf[6]) : (t[0] ? leaf[5] : leaf[4])) : (t[1] ? (t[0] ? leaf[3] : leaf[2]) : (t[0] ? leaf[1] : leaf[0])))));
  endtask
  logic [7:0] m8; logic [3:0] m4; logic signed [5:0] ms; logic [23:0] ymix; logic [7:0] q;
  logic [15:0] ysel;
  initial begin
    for (int i = 0; i <= 4; i++) d4[i] = 16'h8000 + 16'(i * 257);
    for (int pos = 0; pos < 4; pos += 1) begin
      s4 = '1; s4[pos] = 1'b0; calc4();
      $display("left zero n=4 pos=%0d y=%b", pos, l4);
      s4 = '1; s4[pos] = 1'bx; calc4();
      $display("left x n=4 pos=%0d y=%b", pos, l4);
      s4 = '1; s4[pos] = 1'bx; s4[0] = 1'bz; calc4();
      $display("left xz n=4 pos=%0d y=%b", pos, l4);
    end
    for (int i = 0; i <= 8; i++) d8[i] = 16'h8000 + 16'(i * 257);
    for (int pos = 0; pos < 8; pos += 1) begin
      s8 = '1; s8[pos] = 1'b0; calc8();
      $display("left zero n=8 pos=%0d y=%b", pos, l8);
      s8 = '1; s8[pos] = 1'bx; calc8();
      $display("left x n=8 pos=%0d y=%b", pos, l8);
      s8 = '1; s8[pos] = 1'bx; s8[0] = 1'bz; calc8();
      $display("left xz n=8 pos=%0d y=%b", pos, l8);
    end
    for (int i = 0; i <= 16; i++) d16[i] = 16'h8000 + 16'(i * 257);
    for (int pos = 0; pos < 16; pos += 2) begin
      s16 = '1; s16[pos] = 1'b0; calc16();
      $display("left zero n=16 pos=%0d y=%b", pos, l16);
      s16 = '1; s16[pos] = 1'bx; calc16();
      $display("left x n=16 pos=%0d y=%b", pos, l16);
      s16 = '1; s16[pos] = 1'bx; s16[0] = 1'bz; calc16();
      $display("left xz n=16 pos=%0d y=%b", pos, l16);
    end
    for (int i = 0; i <= 32; i++) d32[i] = 16'h8000 + 16'(i * 257);
    for (int pos = 0; pos < 32; pos += 4) begin
      s32 = '1; s32[pos] = 1'b0; calc32();
      $display("left zero n=32 pos=%0d y=%b", pos, l32);
      s32 = '1; s32[pos] = 1'bx; calc32();
      $display("left x n=32 pos=%0d y=%b", pos, l32);
      s32 = '1; s32[pos] = 1'bx; s32[0] = 1'bz; calc32();
      $display("left xz n=32 pos=%0d y=%b", pos, l32);
    end
    for (int i = 0; i < 32; i++) leaf[i] = 16'(i * 2049);
    for (int i = 0; i < 32; i++) begin
      t = 5'(i);
      calct();
      $write("tree %0d y=%h", i, yt);
      t[i % 5] = 1'bx;
      calct();
      $write(" x%0d=%b", i % 5, yt);
      t[(i + 2) % 5] = 1'bz;
      calct();
      $display(" z%0d=%b", (i + 2) % 5, yt);
    end
    m8 = 8'hf0; m4 = 4'ha; ms = -6'sd9;
    for (int i = 0; i < 16; i++) begin
      q = 8'(1 << (i % 8)); if (i >= 8) q[i % 8] = 1'bx;
      ymix = q[0] ? m8 : q[1] ? m4 : q[2] ? ms : q[3] ? {m4, m8} : q[4] ? 24'sd5 - ms : q[5] ? (m8 > 8'd3 ? m4 : ms) : q[6] ? 1'b1 : 'z;
      $display("mix q=%b y=%b", q, ymix);
      ysel = (q[0] ? q[1] : q[2] ? 1'b1 : q[3]) ? 16'haaaa : (q[4] ? 1'bx : q[5] ? 1'b0 : q[6]) ? 16'h5555 : 16'h0ff0;
      $display("psel q=%b y=%b", q, ysel);
    end
  end
endmodule
