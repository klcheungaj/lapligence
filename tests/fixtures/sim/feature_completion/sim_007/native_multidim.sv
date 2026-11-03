// Multidimensional fixed arrays of strings, class handles and records with
// string/real members: element and row selects, patterns, copies, foreach,
// array queries, package and subroutine storage (SV 7.4.5, 7.4.6, 10.9.1,
// 13.5, 26.2).
package nmd_pkg;
  string tbl[2][2] = '{'{"a", "b"}, '{"c", "d"}};
  function automatic string at(int i, int j);
    return tbl[i][j];
  endfunction
endpackage

module tb;
  typedef struct { string s; int n; real r; } rec_t;
  typedef string row_t[3];
  typedef string grid_t[2][2];
  class C;
    int v;
    function new(int x); v = x; endfunction
    function int get(); return v; endfunction
  endclass

  string s[2][3];
  string t[2][3];
  string row[3];
  string d[3:2][1:0];
  string s3[2][2][2];
  row_t tr[2];
  rec_t m[2][2];
  rec_t c[2][2];
  C h[2][2];
  C g[2][2];

  function automatic string join2(string a[2][3]);
    string r = "";
    foreach (a[i, j]) r = {r, a[i][j]};
    return r;
  endfunction
  function automatic void fill(output string a[2][3], input string p);
    foreach (a[i, j]) a[i][j] = $sformatf("%s%0d%0d", p, i, j);
  endfunction
  function automatic int total(rec_t a[2][2]);
    int sum = 0;
    foreach (a[i, j]) sum += a[i][j].n;
    return sum;
  endfunction
  function automatic grid_t mk(string p);
    grid_t q;
    foreach (q[i, j]) q[i][j] = $sformatf("%s%0d%0d", p, i, j);
    return q;
  endfunction
  task automatic swap(ref string a[2][2]);
    string tmp;
    tmp = a[0][0];
    a[0][0] = a[1][1];
    a[1][1] = tmp;
  endtask
  task automatic setrow(output string r[3], input string p);
    foreach (r[i]) r[i] = $sformatf("%s-%0d", p, i);
  endtask
  task automatic tagrow(inout string r[3]);
    foreach (r[i]) r[i] = {r[i], "!"};
  endtask
  function automatic bit rows_equal(string a[3], string b[3]);
    return a == b;
  endfunction

  initial begin
    static string sb[2][2] = '{'{"p", "q"}, '{"r", "t"}};
    automatic string ab[2][2];
    grid_t lg;
    string lo[2][2];
    int k;
    // (A) element writes and reads, whole copies, equality
    foreach (s[i, j]) s[i][j] = $sformatf("s%0d%0d", i, j);
    $display("A1 %s %s %s", s[0][0], s[1][2], s[0][1]);
    t = s;
    t[1][1] = "X";
    $display("A2 %s %s", s[1][1], t[1][1]);
    if (s != t) $display("A3 differ");
    t[1][1] = s[1][1];
    if (s == t) $display("A4 equal");
    // (B) rows: copy out, copy in, row to row, run-time indices
    row = s[1];
    $display("B1 %s %s %s", row[0], row[1], row[2]);
    s[0] = row;
    $display("B2 %s %s", s[0][0], s[0][2]);
    k = 1;
    s[k][k + 1] = "dyn";
    s[k - 1] = s[k];
    $display("B3 %s %s", s[1][2], s[0][2]);
    // (C) nested patterns with positions, keys and defaults
    s = '{'{"a", "b", "c"}, '{default: "z"}};
    $display("C1 %s%s%s %s%s%s", s[0][0], s[0][1], s[0][2], s[1][0], s[1][1], s[1][2]);
    s = '{1: '{2: "k", default: "-"}, default: "d"};
    $display("C2 %s", join2(s));
    // (D) subroutine storage
    $display("D1 %s", join2(s));
    fill(t, "f");
    $display("D2 %s", join2(t));
    lg = mk("x");
    lo = lg;
    lo[0][1] = "y";
    $display("D3 %s %s %s %s", lo[0][0], lo[0][1], lg[0][1], lo[1][1]);
    ab = mk("m");
    swap(ab);
    $display("D4 %s %s %s %s", ab[0][0], ab[0][1], ab[1][0], ab[1][1]);
    $display("D5 %s %s %s %s", sb[0][0], sb[0][1], sb[1][0], sb[1][1]);
    // (E) descending ranges, three dimensions, typedef rows, queries
    foreach (d[i, j]) d[i][j] = $sformatf("d%0d%0d", i, j);
    $display("E1 %s %s %0d %0d %0d", d[3][0], d[2][1], $left(d, 1), $right(d, 2), $size(d, 2));
    foreach (s3[a, b, e]) s3[a][b][e] = $sformatf("%0d%0d%0d", a, b, e);
    s3[1][0] = s3[0][1];
    $display("E2 %s %s %s", s3[1][1][0], s3[1][0][0], s3[1][0][1]);
    foreach (tr[a, b]) tr[a][b] = $sformatf("t%0d%0d", a, b);
    $display("E3 %s %s %0d %0d", tr[1][2], tr[0][0], rows_equal(tr[0], tr[1]), rows_equal(tr[1], tr[1]));
    setrow(tr[0], "o");
    tagrow(tr[k]);
    $display("E4 %s %s %s %s", tr[0][0], tr[0][2], tr[1][0], tr[1][2]);
    $display("E5 %s %0d %s", tr[1][2].toupper(), tr[1][2].len(), d[3][1].substr(0, 0));
    // (F) records with string and real members
    foreach (m[i, j]) begin
      m[i][j].s = $sformatf("m%0d%0d", i, j);
      m[i][j].n = i * 10 + j;
      m[i][j].r = real'(i) + real'(j) * 0.5;
    end
    $display("F1 %s %0d %f", m[1][0].s, m[1][0].n, m[1][1].r);
    c = m;
    c[0][1].s = "chg";
    $display("F2 %s %s", m[0][1].s, c[0][1].s);
    if (c != m) $display("F3 differ");
    $display("F4 total=%0d", total(m));
    m[1][1] = '{s: "pat", n: 99, r: 1.5};
    m[k][0].n++;
    $display("F5 %s %0d %f %0d", m[1][1].s, m[1][1].n, m[1][1].r, m[1][0].n);
    m[0] = c[1];
    $display("F6 %s %s", m[0][0].s, m[0][1].s);
    if (m[1][0] == c[1][0]) $display("F7 same");
    // (G) class handles: identity copies
    foreach (h[i, j]) h[i][j] = new(i * 2 + j);
    $display("G1 %0d %0d", h[0][1].v, h[1][1].get());
    g = h;
    g[1][1].v = 77;
    $display("G2 %0d", h[1][1].v);
    if (g[1][1] == h[1][1]) $display("G3 same object");
    h[0][0] = null;
    if (h[0][0] == null && g != h) $display("G4 null");
    foreach (h[i, j]) if (h[i][j] != null) h[i][j].v++;
    $display("G5 %0d %0d", h[1][0].v, g[1][0].v);
    $display("H %p", sb);
    // (I) package storage
    nmd_pkg::tbl[1][0] = "C";
    $display("I %s %s %s", nmd_pkg::at(1, 0), nmd_pkg::tbl[0][1], nmd_pkg::tbl[k][k]);
    $finish(0);
  end
endmodule
