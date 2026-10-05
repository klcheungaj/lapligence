// IEEE 1800-2009 10.10 with descriptor storage (more than 4096 cells) on
// either side: a small source into descriptor row targets, and a nested
// descriptor source into dense and descriptor rows. Rows are copied, never
// flattened, and every selector is evaluated once.
module tb;
  typedef logic [7:0] row_t [4];
  typedef row_t two_t [2];
  logic [7:0] w [2][4];
  logic [7:0] big [2000][4];
  logic [7:0] d [4];
  logic [7:0] h0, h1, h2, h3;
  logic [7:0] src [2][2][3000];
  logic [7:0] a [3000], b [3000], e [3000], f [3000];
  logic [7:0] rows [8][3000];
  int k;

  function automatic int next_k();
    k++;
    return k;
  endfunction

  initial begin
    for (int i = 0; i < 2; i++)
      for (int j = 0; j < 4; j++) w[i][j] = 8'(16 * i + j);
    k = 5;
    '{big[k], d} = w;
    $display("row %h %h %h %h | %h %h %h %h", big[5][0], big[5][1], big[5][2], big[5][3], d[0], d[1], d[2], d[3]);
    '{big[next_k()], '{h0, h1, h2, h3}} = w;
    $display("leaves %0d %h %h | %h %h %h %h", k, big[6][0], big[6][3], h0, h1, h2, h3);
    '{'{h3, h2, h1, h0}, big[k]} <= w;
    $display("before %h %h", h0, big[6][3]);
    #1 $display("nba %h %h %h %h | %h %h", h0, h1, h2, h3, big[6][0], big[6][3]);
    '{big[1999], big[0]} = two_t'{default: 8'h5a};
    $display("default %h %h %h", big[1999][0], big[0][3], big[1][0]);
    '{d, big[1999]} = two_t'{'{1, 2, 3, 4}, d};
    $display("overlap %h %h | %h %h", d[0], d[3], big[1999][0], big[1999][3]);
    for (int i = 0; i < 2; i++)
      for (int j = 0; j < 2; j++)
        for (int l = 0; l < 3000; l++) src[i][j][l] = 8'(64 * i + 16 * j + l);
    '{'{a, b}, '{e, f}} = src;
    $display("dense %h %h | %h %h | %h %h | %h %h", a[0], a[2999], b[1], b[2999], e[2], e[2999], f[3], f[2999]);
    '{'{rows[3], rows[7]}, '{e, rows[0]}} = src;
    $display("rows %h %h | %h %h | %h %h | %h %h", rows[3][0], rows[3][2999], rows[7][1], rows[7][2999], e[2], e[2999], rows[0][3], rows[0][2999]);
    '{'{a, rows[1]}, '{rows[2], f}} <= src;
    #1 $display("nested nba %h %h %h %h", a[4], rows[1][5], rows[2][6], f[7]);
    $finish(0);
  end
endmodule
