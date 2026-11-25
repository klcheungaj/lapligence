// Memory images whose text, addresses or digits do not fit (21.4).
module tb;
  logic [7:0] m[0:7], off[4:7];
  logic [0:7] mm[0:1][0:1];
  logic [69:0] w[0:1];
  bit [3:0] n[7:0];
  int it[0:3];
  integer f;
  int ctx;

  task init;
    foreach (m[i]) m[i] = 8'h11;
    foreach (n[i]) n[i] = 5;
    foreach (it[i]) it[i] = 9;
    foreach (w[i]) w[i] = 3;
    foreach (off[i]) off[i] = 8'h22;
    foreach (mm[i, j]) mm[i][j] = 8'h33;
  endtask

  task wr(string t);
    f = $fopen("data.txt", "w");
    $fwrite(f, "%s", t);
    $fclose(f);
  endtask

  task show;
    foreach (m[i]) $write("%h ", m[i]);
    foreach (n[i]) $write("%h ", n[i]);
    foreach (it[i]) $write("%0d ", it[i]);
    foreach (w[i]) $write("%h ", w[i]);
    foreach (off[i]) $write("%h ", off[i]);
    foreach (mm[i,j]) $write("%h ", mm[i][j]);
    $display;
  endtask

  initial begin
    void'($value$plusargs("ctx=%d", ctx));
    init;
    case (ctx)
      0: begin wr("12 34\n"); $readmemh("data.txt", m); end
      1: begin wr("12 3g 56\n"); $readmemh("data.txt", m); end
      2: begin wr("123 4567\n"); $readmemh("data.txt", m); end
      3: begin wr("1 2 3 4 5 6 7 8 9 a\n"); $readmemh("data.txt", m); end
      4: begin wr("@2 55 @9 66\n"); $readmemh("data.txt", m); end
      5: begin wr("@zz 55\n"); $readmemh("data.txt", m); end
      6: begin wr("@ 2 55\n"); $readmemh("data.txt", m); end
      7: begin wr("12 // c\n34 /* x\n y */ 56"); $readmemh("data.txt", m); end
      8: begin wr("12 /* unterminated"); $readmemh("data.txt", m); end
      9: begin wr("x z 1x z_1 _ 1_\n"); $readmemh("data.txt", m); end
      10: begin wr("12 102 1x\n"); $readmemb("data.txt", m); end
      11: begin wr("101010101 11\n"); $readmemb("data.txt", m); end
      12: begin wr("-1 +2\n"); $readmemh("data.txt", m); end
      13: begin wr("8'h12 'h3\n"); $readmemh("data.txt", m); end
      14: begin $readmemh("missing.txt", m); end
      15: begin $readmemh("", m); end
      16: begin wr("1 2 3 4\n"); $readmemh("data.txt", m, 5, 2); end
      17: begin wr("1 2\n"); $readmemh("data.txt", m, 2, 5); end
      18: begin wr("1 2 3 4 5 6\n"); $readmemh("data.txt", m, 2, 5); end
      19: begin wr("@0 1 2\n"); $readmemh("data.txt", m, 2, 5); end
      20: begin wr("1 2\n"); $readmemh("data.txt", m, 6, 12); end
      21: begin wr("1 2\n"); $readmemh("data.txt", m, -1); end
      22: begin wr("1 2\n"); $readmemh("data.txt", m, 4'bx); end
      23: begin wr("x z 9\n"); $readmemh("data.txt", n); end
      24: begin wr("7fffffff 80000000 1ffffffff\n"); $readmemh("data.txt", it); end
      25: begin wr("3fffffffffffffffff 7fffffffffffffffff\n"); $readmemh("data.txt", w); end
      26: begin wr("1 2 3\n"); $readmemh("data.txt", off); end
      27: begin wr("@0 1 2 3\n"); $readmemh("data.txt", off); end
      28: begin wr("@5 1 2 3 4\n"); $readmemh("data.txt", off); end
      29: begin wr("1 2 3 4 5\n"); $readmemh("data.txt", mm); end
      30: begin wr("\000 1 2\n"); $readmemh("data.txt", m); end
      31: begin wr("1,2,3\n"); $readmemh("data.txt", m); end
      32: begin wr("1 @3\n"); $readmemh("data.txt", m); end
      33: begin wr(""); $readmemh("data.txt", m); end
      34: begin wr("// only\n"); $readmemh("data.txt", m); end
      35: begin wr("@ffffffffffffffffff 1\n"); $readmemh("data.txt", m); end
      36: begin wr("@-1 1\n"); $readmemh("data.txt", m); end
      37: begin wr("@x 1\n"); $readmemh("data.txt", m); end
      38: begin wr("? 1? \n"); $readmemh("data.txt", m); end
      39: begin wr("1 2\r\n3\r\n"); $readmemh("data.txt", m); end
      40: begin wr("1\f2\v3\t4\n"); $readmemh("data.txt", m); end
      41: begin wr("# 1\n"); $readmemh("data.txt", m); end
      42: begin wr("@1@2 5\n"); $readmemh("data.txt", m); end
      43: begin wr("@3 5 @1 6 7 8 9\n"); $readmemh("data.txt", m); end
    endcase
    show;
  end
endmodule
