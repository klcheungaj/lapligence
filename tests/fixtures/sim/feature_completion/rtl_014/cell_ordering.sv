// IEEE 1800-2009 7.12.2: in-place ordering of stored fixed-array cells.
// Every receiver here has more elements than the straight-line limit, so
// the cell-wise loop runs; keys are unique, so the order is exact.
module tb;
  typedef struct packed {
    logic [3:0] k;
    logic [7:0] v;
  } rec_t;
  typedef enum logic [2:0] {E0 = 3'd6, E1 = 3'd2, E2 = 3'd4, E3 = 3'd0} e_t;

  byte signed s [20:1];
  int unsigned u [-3:16];
  rec_t r [0:19];
  int m [0:17][0:2];
  int g [0:2][0:19];
  logic [7:0] x [0:19];
  e_t e [0:19];
  integer idx;

  function automatic void sort_ref(ref int a [0:19]);
    a.sort();
  endfunction

  function automatic void sort_keyed(ref int a [0:19], input int k);
    a.rsort() with (item ^ k);
  endfunction

  function automatic int local_order(int k);
    int l [0:19];
    foreach (l[i]) l[i] = 19 - i;
    l.sort() with (item * k + item.index);
    return l[0] * 100 + l[19];
  endfunction

  initial begin
    foreach (s[i]) s[i] = byte'(i * 53);
    s.sort();
    $write("s.sort:");
    foreach (s[i]) $write(" %0d", s[i]);
    $display();
    s.rsort();
    $write("s.rsort:");
    foreach (s[i]) $write(" %0d", s[i]);
    $display();
    s.reverse();
    $write("s.reverse:");
    foreach (s[i]) $write(" %0d", s[i]);
    $display();

    foreach (u[i]) u[i] = (i + 7) * 32'd2654435761;
    u.sort() with (~item);
    $write("u.key:");
    foreach (u[i]) $write(" %0d", u[i] % 1000);
    $display();
    u.sort() with (item.index * -1);
    $write("u.index:");
    foreach (u[i]) $write(" %0d", u[i] % 1000);
    $display();

    foreach (r[i]) r[i] = '{k: 4'(i * 7), v: 8'(i)};
    r.rsort() with ({item.k, item.v});
    $write("r.rsort:");
    foreach (r[i]) $write(" %0d:%0d", r[i].k, r[i].v);
    $display();
    r.sort();
    $write("r.sort:");
    foreach (r[i]) $write(" %0d", r[i].v);
    $display();

    foreach (m[i, j]) m[i][j] = (i * 5 + j * 3) % 19 + j * 100;
    m.sort() with (item[1]);
    $write("m.rows:");
    foreach (m[i]) $write(" %0d/%0d/%0d", m[i][0], m[i][1], m[i][2]);
    $display();
    m.reverse();
    $display("m.reverse: %0d %0d", m[0][1], m[17][1]);

    foreach (g[i, j]) g[i][j] = (j * 7) % 20;
    sort_ref(g[0]);
    $write("g0:");
    foreach (g[0][j]) $write(" %0d", g[0][j]);
    $display();
    sort_keyed(g[1], 5);
    $write("g1:");
    foreach (g[1][j]) $write(" %0d", g[1][j]);
    $display();
    idx = 2;
    g[idx].reverse();
    $write("g2:");
    foreach (g[2][j]) $write(" %0d", g[2][j]);
    $display();
    idx = 'x;
    g[idx].sort();
    g[idx + 5].rsort();
    $display("unselected: %0d %0d", g[2][0], g[2][19]);
    $display("local: %0d", local_order(3));

    foreach (e[i]) begin
      case ((i * 3) % 4)
        0: e[i] = E0;
        1: e[i] = E1;
        2: e[i] = E2;
        default: e[i] = E3;
      endcase
    end
    e.sort() with (int'(item) * 32 + item.index);
    $write("e:");
    foreach (e[i]) $write(" %0d", e[i]);
    $display();

    // Unknown-key order is unspecified; the result is still a permutation.
    foreach (x[i]) x[i] = (i % 5 == 0) ? 8'bx : 8'(40 - i);
    x.sort();
    $display("x: unknown=%0d known=%0d", x.sum() with (int'($isunknown(item))),
             x.sum() with ($isunknown(item) ? 0 : int'(item)));
    $finish(0);
  end
endmodule
