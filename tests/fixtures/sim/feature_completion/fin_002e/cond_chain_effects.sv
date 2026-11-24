// Evaluation order and side effects in conditional chains: a known predicate
// evaluates one alternative, an ambiguous one both (IEEE 1800-2009 11.4.11).
`timescale 1ns/1ns
module tb;
  string log;
  int hits [40];
  function automatic logic sel(int id, logic v);
    log = {log, $sformatf("s%0d ", id)};
    return v;
  endfunction
  function automatic logic [15:0] arm(int id);
    log = {log, $sformatf("a%0d ", id)};
    hits[id]++;
    return 16'h1100 + 16'(id);
  endfunction
  logic [15:0] y;
  logic [3:0] c4;
  task automatic calc4();
    y = sel(0, c4[0]) ? arm(0) : sel(1, c4[1]) ? arm(1) : sel(2, c4[2]) ? arm(2) : sel(3, c4[3]) ? arm(3) : arm(4);
  endtask
  logic [7:0] c8;
  task automatic calc8();
    y = sel(0, c8[0]) ? arm(0) : sel(1, c8[1]) ? arm(1) : sel(2, c8[2]) ? arm(2) : sel(3, c8[3]) ? arm(3) : sel(4, c8[4]) ? arm(4) : sel(5, c8[5]) ? arm(5) : sel(6, c8[6]) ? arm(6) : sel(7, c8[7]) ? arm(7) : arm(8);
  endtask
  logic [15:0] c16;
  task automatic calc16();
    y = sel(0, c16[0]) ? arm(0) : sel(1, c16[1]) ? arm(1) : sel(2, c16[2]) ? arm(2) : sel(3, c16[3]) ? arm(3) : sel(4, c16[4]) ? arm(4) : sel(5, c16[5]) ? arm(5) : sel(6, c16[6]) ? arm(6) : sel(7, c16[7]) ? arm(7) : sel(8, c16[8]) ? arm(8) : sel(9, c16[9]) ? arm(9) : sel(10, c16[10]) ? arm(10) : sel(11, c16[11]) ? arm(11) : sel(12, c16[12]) ? arm(12) : sel(13, c16[13]) ? arm(13) : sel(14, c16[14]) ? arm(14) : sel(15, c16[15]) ? arm(15) : arm(16);
  endtask
  logic [31:0] c32;
  task automatic calc32();
    y = sel(0, c32[0]) ? arm(0) : sel(1, c32[1]) ? arm(1) : sel(2, c32[2]) ? arm(2) : sel(3, c32[3]) ? arm(3) : sel(4, c32[4]) ? arm(4) : sel(5, c32[5]) ? arm(5) : sel(6, c32[6]) ? arm(6) : sel(7, c32[7]) ? arm(7) : sel(8, c32[8]) ? arm(8) : sel(9, c32[9]) ? arm(9) : sel(10, c32[10]) ? arm(10) : sel(11, c32[11]) ? arm(11) : sel(12, c32[12]) ? arm(12) : sel(13, c32[13]) ? arm(13) : sel(14, c32[14]) ? arm(14) : sel(15, c32[15]) ? arm(15) : sel(16, c32[16]) ? arm(16) : sel(17, c32[17]) ? arm(17) : sel(18, c32[18]) ? arm(18) : sel(19, c32[19]) ? arm(19) : sel(20, c32[20]) ? arm(20) : sel(21, c32[21]) ? arm(21) : sel(22, c32[22]) ? arm(22) : sel(23, c32[23]) ? arm(23) : sel(24, c32[24]) ? arm(24) : sel(25, c32[25]) ? arm(25) : sel(26, c32[26]) ? arm(26) : sel(27, c32[27]) ? arm(27) : sel(28, c32[28]) ? arm(28) : sel(29, c32[29]) ? arm(29) : sel(30, c32[30]) ? arm(30) : sel(31, c32[31]) ? arm(31) : arm(32);
  endtask
  logic u;
  initial begin
    for (int pos = 0; pos < 4; pos += 1) begin
      c4 = '0; c4[pos] = 1'b1; log = ""; calc4();
      $display("one n=4 pos=%0d y=%h : %s", pos, y, log);
      c4 = '0; c4[pos] = 1'bx; log = ""; calc4();
      $display("x n=4 pos=%0d y=%h : %s", pos, y, log);
      c4 = '0; c4[pos] = 1'bx; c4[3] = 1'bz; log = ""; calc4();
      $display("xz n=4 pos=%0d y=%h : %s", pos, y, log);
      c4 = '0; c4[pos] = 1'bx; if (pos + 1 < 4) c4[pos + 1] = 1'b1; log = ""; calc4();
      $display("x1 n=4 pos=%0d y=%h : %s", pos, y, log);
    end
    for (int pos = 0; pos < 8; pos += 1) begin
      c8 = '0; c8[pos] = 1'b1; log = ""; calc8();
      $display("one n=8 pos=%0d y=%h : %s", pos, y, log);
      c8 = '0; c8[pos] = 1'bx; log = ""; calc8();
      $display("x n=8 pos=%0d y=%h : %s", pos, y, log);
      c8 = '0; c8[pos] = 1'bx; c8[7] = 1'bz; log = ""; calc8();
      $display("xz n=8 pos=%0d y=%h : %s", pos, y, log);
      c8 = '0; c8[pos] = 1'bx; if (pos + 1 < 8) c8[pos + 1] = 1'b1; log = ""; calc8();
      $display("x1 n=8 pos=%0d y=%h : %s", pos, y, log);
    end
    for (int pos = 0; pos < 16; pos += 2) begin
      c16 = '0; c16[pos] = 1'b1; log = ""; calc16();
      $display("one n=16 pos=%0d y=%h : %s", pos, y, log);
      c16 = '0; c16[pos] = 1'bx; log = ""; calc16();
      $display("x n=16 pos=%0d y=%h : %s", pos, y, log);
      c16 = '0; c16[pos] = 1'bx; c16[15] = 1'bz; log = ""; calc16();
      $display("xz n=16 pos=%0d y=%h : %s", pos, y, log);
      c16 = '0; c16[pos] = 1'bx; if (pos + 1 < 16) c16[pos + 1] = 1'b1; log = ""; calc16();
      $display("x1 n=16 pos=%0d y=%h : %s", pos, y, log);
    end
    for (int pos = 0; pos < 32; pos += 4) begin
      c32 = '0; c32[pos] = 1'b1; log = ""; calc32();
      $display("one n=32 pos=%0d y=%h : %s", pos, y, log);
      c32 = '0; c32[pos] = 1'bx; log = ""; calc32();
      $display("x n=32 pos=%0d y=%h : %s", pos, y, log);
      c32 = '0; c32[pos] = 1'bx; c32[31] = 1'bz; log = ""; calc32();
      $display("xz n=32 pos=%0d y=%h : %s", pos, y, log);
      c32 = '0; c32[pos] = 1'bx; if (pos + 1 < 32) c32[pos + 1] = 1'b1; log = ""; calc32();
      $display("x1 n=32 pos=%0d y=%h : %s", pos, y, log);
    end
    for (int id = 0; id <= 32; id++) $display("hits[%0d]=%0d", id, hits[id]);
    u = 1'b1; log = "";
    y = (sel(0, u) && sel(1, 1'b1)) ? arm(33) : (sel(2, u) || sel(3, 1'b0)) ? arm(34) : arm(35);
    $display("sc u=%b y=%h : %s", u, y, log);
    u = 1'b0; log = "";
    y = (sel(0, u) && sel(1, 1'b1)) ? arm(33) : (sel(2, u) || sel(3, 1'b0)) ? arm(34) : arm(35);
    $display("sc u=%b y=%h : %s", u, y, log);
    u = 1'bx; log = "";
    y = (sel(0, u) && sel(1, 1'b1)) ? arm(33) : (sel(2, u) || sel(3, 1'b0)) ? arm(34) : arm(35);
    $display("sc u=%b y=%h : %s", u, y, log);
    u = 1'bz; log = "";
    y = (sel(0, u) && sel(1, 1'b1)) ? arm(33) : (sel(2, u) || sel(3, 1'b0)) ? arm(34) : arm(35);
    $display("sc u=%b y=%h : %s", u, y, log);
  end
endmodule
