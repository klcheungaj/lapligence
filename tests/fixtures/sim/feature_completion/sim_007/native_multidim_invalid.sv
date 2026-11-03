// Invalid indices of multidimensional native arrays (SV 7.4.6): reads give
// the element type's default and writes do nothing.
module tb;
  typedef struct { string s; int n; } rec_t;
  class C; int v = 5; endclass
  string s[2][3];
  rec_t m[3:2][0:1];
  C h[2][2];
  string row[3];
  initial begin
    logic [1:0] x;
    int k;
    foreach (s[i, j]) s[i][j] = $sformatf("s%0d%0d", i, j);
    foreach (m[i, j]) m[i][j] = '{s: "m", n: i + j};
    foreach (h[i, j]) h[i][j] = new;
    x = 2'b1x;
    k = 3;
    $display("1 [%s] [%s] [%s]", s[x][0], s[0][x], s[k][0]);
    $display("2 [%s] %0d", m[1][0].s, m[k][0].n);
    $display("3 %0d %0d", h[0][k] == null, h[1][1].v);
    s[k][0] = "no";
    s[0][x] = "no";
    m[1][1].s = "no";
    $display("4 %s %s %s %s", s[0][0], s[1][0], s[0][1], m[2][1].s);
    row = '{"r0", "r1", "r2"};
    row = s[k];
    $display("5 [%s] [%s] [%s]", row[0], row[1], row[2]);
    s[k] = '{"w", "w", "w"};
    $display("6 %s %s", join_all(), s[1][2]);
    $finish(0);
  end
  function automatic string join_all();
    string r = "";
    foreach (s[i, j]) r = {r, s[i][j]};
    return r;
  endfunction
endmodule
