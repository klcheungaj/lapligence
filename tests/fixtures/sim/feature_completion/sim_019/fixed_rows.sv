// FIN-001 G3 (F07-13): locator, min/max, unique and shuffle methods on a
// selected row of a multidimensional fixed array (SV 7.12.1, 7.12.2,
// 7.4.5). Row selectors are evaluated once; index results report the
// row's declared indices.
module tb;
  int m[2][3] = '{'{3, 1, 2}, '{9, 7, 8}};
  logic [7:0] d[1:0][4:2];
  int c[2][2][3];
  real rr[2][3];
  int r[$];
  real q[$];
  int calls;
  function automatic int pick(int v);
    calls++;
    return v;
  endfunction
  initial begin
    d[0] = '{8'h11, 8'h22, 8'h11};
    d[1] = '{8'h05, 8'h06, 8'h07};
    foreach (c[i, j, k]) c[i][j][k] = i * 100 + j * 10 + k;
    rr[1] = '{1.5, -2.0, 4.25};
    r = m[pick(1)].find(x) with (x > 7);
    $display("find %0d: %0d calls %0d", r.size(), r.sum(), calls);
    r = m[pick(0)].find_last_index(x) with (x < 3);
    $display("fli %0d: %0d calls %0d", r.size(), r[0], calls);
    r = m[1].max();
    $display("max %0d", r[0]);
    r = m[0].min() with (item * -1);
    $display("minw %0d", r[0]);
    r = d[0].unique_index();
    $display("ui %0d", r.size());
    r = d[0].find_first_index(x) with (x == 8'h11);
    $display("ffi %0d", r[0]);
    r = d[1].find_index with (item > 8'h05);
    $display("idx %0d: %0d", r.size(), r.sum());
    r = c[1][0].find_first with (item > 100);
    $display("3d %0d", r[0]);
    q = rr[1].min();
    $display("real %0.2f", q[0]);
    m[pick(1)].shuffle();
    $display("shuffle %0d %0d calls %0d", m[1].sum(), m[1].product(), calls);
    $display("rows %0d %0d", m[0].sum(), d[0].sum() with (int'(item)));
    $finish(0);
  end
endmodule
