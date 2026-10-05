// IEEE 1800-2009 10.10: a positional pattern lvalue takes its source's
// elements in declaration order; a target that is itself an unpacked row
// receives a whole row. Every source here is small dense storage.
module tb;
  typedef logic [7:0] row_t [4];
  typedef row_t two_t [2];
  typedef struct { row_t r; logic [3:0] x; } rec_t;
  logic [7:0] w [2][4];
  logic [7:0] wd [1:0][3:0];
  logic [7:0] c [4], d [4];
  logic [7:0] cd [0:3], dd [3:0];
  logic [3:0][1:0] pw [2][3];
  logic [3:0][1:0] pa [3], pb [3];
  logic [7:0] w3 [2][2][3];
  logic [7:0] a [3], b [3], e [3], f [3];
  logic [7:0] g [2][3];
  logic [7:0] h0, h1, h2;
  row_t p, q;
  rec_t s;
  logic [3:0] x;
  int k;

  function automatic two_t mk(int base);
    for (int i = 0; i < 2; i++)
      for (int j = 0; j < 4; j++) mk[i][j] = 8'(base + 16 * i + j);
  endfunction

  function automatic int next_k();
    k++;
    return k;
  endfunction

  initial begin
    for (int i = 0; i < 2; i++)
      for (int j = 0; j < 4; j++) begin
        w[i][j] = 8'(16 * i + j);
        wd[i][j] = 8'(16 * i + j);
      end
    '{c, d} = w;
    $display("rows %h %h %h %h | %h %h %h %h", c[0], c[1], c[2], c[3], d[0], d[1], d[2], d[3]);
    '{cd, dd} = wd;
    $display("desc %h %h | %h %h", cd[0], cd[3], dd[3], dd[0]);
    '{c, d} = two_t'{d, c};
    $display("swap %h %h | %h %h", c[0], c[3], d[0], d[3]);
    for (int i = 0; i < 2; i++)
      for (int j = 0; j < 3; j++) pw[i][j] = 8'(32 * i + j + 1);
    '{pa, pb} = pw;
    $display("packed %h %h %h | %h %h %h", pa[0], pa[1], pa[2], pb[0], pb[1], pb[2]);
    for (int i = 0; i < 2; i++)
      for (int j = 0; j < 2; j++)
        for (int l = 0; l < 3; l++) w3[i][j][l] = 8'(64 * i + 16 * j + l);
    '{'{a, b}, '{e, f}} = w3;
    $display("nested %h %h | %h %h | %h %h | %h %h", a[0], a[2], b[0], b[2], e[0], e[2], f[0], f[2]);
    '{'{a, '{h0, h1, h2}}, g} = w3;
    $display("mixed %h %h | %h %h %h | %h %h", a[0], a[2], h0, h1, h2, g[0][0], g[1][2]);
    '{c, d} = two_t'{'{1, 2, 3, 4}, '{5, 6, 7, 8}};
    $display("typed %h %h | %h %h", c[0], c[3], d[0], d[3]);
    p = '{8'ha0, 8'ha1, 8'ha2, 8'ha3};
    q = '{8'hb0, 8'hb1, 8'hb2, 8'hb3};
    '{c, d} = two_t'{q, p};
    $display("items %h %h | %h %h", c[0], c[3], d[0], d[3]);
    '{c, d} = mk(32);
    $display("call %h %h | %h %h", c[0], c[3], d[0], d[3]);
    '{c, d} = two_t'{default: 8'h5a};
    $display("default %h %h | %h %h", c[0], c[3], d[0], d[3]);
    s.r = p;
    s.x = 4'h9;
    '{c, x} = s;
    $display("record %h %h | %h", c[0], c[3], x);
    w[0][1] = 8'bxxxx_0101;
    '{c, d} <= w;
    $display("before %h %h", c[1], d[1]);
    #1 $display("nba %h %h %h", c[1], d[1], d[3]);
    k = 0;
    '{w[next_k()], c} = wd;
    $display("selector %0d %h %h %h", k, w[1][0], w[1][3], c[0]);
    $finish(0);
  end
endmodule
