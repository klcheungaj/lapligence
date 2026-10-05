// IEEE 1800-2009 10.3 and 10.10: continuous positional pattern lvalues with
// row targets. Each row is one driver of its variable or net; the drivers
// re-evaluate when any source cell changes.
module tb;
  typedef logic [7:0] row_t [4];
  typedef row_t two_t [2];
  logic [7:0] w [2][4];
  logic [7:0] vc [4], vd [4];
  wire [7:0] nc [4], nd [4];
  wire [7:0] n0, n1, n2, n3;
  logic [7:0] big [2000][4];
  logic [7:0] h0, h1, h2, h3;
  logic [7:0] src [2][2][3000];
  logic [7:0] a [3000], b [3000], e [3000];
  logic [7:0] rows [8][3000];

  assign '{vc, vd} = w;
  assign '{nc, '{n0, n1, n2, n3}} = w;
  assign '{big[7], '{h0, h1, h2, h3}} = w;
  assign '{'{a, b}, '{e, rows[6]}} = src;
  assign '{nd, big[8]} = two_t'{w[1], w[0]};

  initial begin
    for (int i = 0; i < 2; i++)
      for (int j = 0; j < 4; j++) w[i][j] = 8'(16 * i + j);
    for (int i = 0; i < 2; i++)
      for (int j = 0; j < 2; j++)
        for (int l = 0; l < 3000; l++) src[i][j][l] = 8'(64 * i + 16 * j + l);
    #1;
    $display("var %h %h | %h %h", vc[0], vc[3], vd[0], vd[3]);
    $display("net %h %h | %h %h %h %h", nc[0], nc[3], n0, n1, n2, n3);
    $display("desc %h %h | %h %h %h %h", big[7][0], big[7][3], h0, h1, h2, h3);
    $display("src %h %h | %h %h | %h %h | %h %h", a[0], a[2999], b[1], b[2999], e[2], e[2999], rows[6][3], rows[6][2999]);
    $display("items %h %h | %h %h", nd[0], nd[3], big[8][0], big[8][3]);
    w[0][1] = 8'hdd;
    w[1][2] = 8'hee;
    src[1][1][5] = 8'haa;
    src[0][0][0] = 8'hbb;
    #1;
    $display("update %h %h %h %h %h %h", vc[1], vd[2], nc[1], n2, big[7][1], h2);
    $display("update %h %h %h %h", a[0], rows[6][5], nd[2], big[8][1]);
    $finish(0);
  end
endmodule
