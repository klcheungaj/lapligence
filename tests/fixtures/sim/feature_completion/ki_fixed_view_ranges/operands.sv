// Elements of fixed arrays of queues, dynamic and associative arrays as
// whole operands: equality (7.4.3), items of unpacked array concatenations
// (10.10) and actuals of `ref` formals of functions (13.5.2).
class C;
  int q[2][$];
  string s[2][$];
endclass

module tb;
  int q[2][$];
  int t[$];
  int m[2][int];
  int r[2][2][$];
  real rq[1:0][$];
  string sq[2][$];
  int d[1:2][];
  int n;

  function automatic int f(int x);
    int a[2][$];
    a[0] = '{x, x + 1};
    a[1] = {a[0], 3};
    return (a[0] == a[1]) + 2 * (a[0] != a[1]) + 4 * a[1].size();
  endfunction

  function automatic void push(ref int x[$], input int v);
    x.push_back(v);
  endfunction

  function automatic int count(const ref int x[$]);
    return x.size();
  endfunction

  function automatic void grow(ref int x[]);
    x = new[3](x);
  endfunction

  initial begin
    C c = new;
    q[0] = '{1, 2};
    q[1] = '{1, 2};
    if (q[0] == q[1]) $display("eq");
    n = (q[0] != q[1]) ? 5 : 6;
    $display("%0d", n);
    t = '{1, 2};
    $display("%0d %0d %0d", q[0] == t, t == q[1], q[0] === q[1]);
    q[1] = {q[0], q[1]};
    $display("%p", q);
    t = {q[0], 9, q[1]};
    $display("%p", t);
    m[0][3] = 1;
    m[1][3] = 1;
    $display("%0d", m[0] == m[1]);
    m[1][4] = 2;
    $display("%0d", m[0] != m[1]);
    r[0][1] = '{7};
    r[1][0] = '{7};
    $display("%0d %0d", r[0][1] == r[1][0], r[0][0] == r[1][1]);
    r[1][1] = {r[0][1], r[1][0]};
    $display("%p %p %p", r[0][1], r[1][0], r[1][1]);
    rq[1] = '{1.5};
    rq[0] = {rq[1], 2.5};
    $display("%p %0d", rq, rq[0] == rq[1]);
    sq[0] = '{"a"};
    sq[1] = {sq[0], "b"};
    $display("%p %0d", sq, sq[0] != sq[1]);
    c.q[0] = '{4};
    c.q[1] = {c.q[0], c.q[0]};
    $display("%p %0d", c.q, c.q[0] == c.q[1]);
    c.s[1] = {"x", "y"};
    c.s[0] = {c.s[1]};
    $display("%p %0d", c.s, c.s[0] == c.s[1]);
    $display("%0d", f(3));
    q[0] = q[1];
    q[0].pop_back();
    q[0].pop_back();
    while (q[0] != q[1]) q[0].push_back(q[1][q[0].size()]);
    $display("%p", q);
    push(q[1], 8);
    push(r[1][0], 5);
    $display("%p %p %0d", q, r[1][0], count(q[1]));
    push(c.q[0], 3);
    $display("%p", c.q);
    d = '{'{1}, '{2, 3}};
    grow(d[1]);
    $display("%p", d);
  end
endmodule
