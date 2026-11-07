module tb;
  typedef int iq_t[$];
  typedef string sq_t[$];
  int q[$];
  int n;
  int qq[$][$];
  int total_calls;
  function automatic iq_t twice(iq_t in);
    foreach (in[i]) twice.push_back(in[i] * 2);
    n++;
  endfunction
  function automatic int sum(iq_t in);
    return in.sum();
  endfunction
  function automatic int cnt(string s[$]);
    return s.size();
  endfunction
  function automatic sq_t words();
    words.push_back("a");
  endfunction
  function automatic int f(int a[]);
    return a.size();
  endfunction
  class C;
    int v[$];
    function int s(); return sum(v); endfunction
    function int t(int k); if (sum(v) > k) return sum(twice(v)); return 0; endfunction
  endclass
  int d[];
  C c;
  initial begin
    c = new;
    q = '{1, 2, 3};
    d = new[2];
    if (sum(q[1:2]) == 5) $display("slice");
    if (sum(twice(q)) == 12) $display("twice");
    if (cnt(words()) == 1) $display("words %0d", cnt(words()));
    while (sum(twice(q)) > 12) q.delete(0);
    $display("%0d", sum({q, 4}));
    c.v = q;
    if (sum(c.v) > 0) $display("%0d %0d", c.s(), c.t(1));
    if (q.size() > 100 && sum(twice(q)) > 0) $display("no");
    $display("n=%0d", n);
    if (f(d) == 2) $display("dyn");
    qq.push_back(q);
    if (sum(qq[0]) == 6) $display("elem");
    if (f(q) == 3) $display("q as dyn");
    $finish;
  end
endmodule
