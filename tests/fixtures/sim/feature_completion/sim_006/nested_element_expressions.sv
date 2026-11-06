// Methods and selects of queues nested in container elements, in any
// expression (SV 7.4.5, 7.8, 7.10.2); output and inout actuals (SV 13.5.2).
module tb;
  int q[$][$];
  int d[][];
  int sq[string][$];
  int iq[int][$];
  int r;
  function automatic int f(int qq[$][$]);
    return qq[0].sum();
  endfunction
  function automatic int g(int i);
    return q[i].sum();
  endfunction
  function automatic int p();
    return q[0].pop_front();
  endfunction
  function automatic void wr(output int o[$]);
    o = '{4, 5};
  endfunction
  function automatic void io(inout int o[$]);
    o.push_back(6);
  endfunction
  function automatic int sum(int o[$]);
    return o.sum();
  endfunction
  initial begin
    q.push_back('{1, 2});
    q.push_back('{3});
    d = new[2];
    d[1] = '{1, 2, 3};
    if (q[0].pop_front() == 1) $display("one");
    $display("%p", q);
    $display("%0d %0d", g(0), p());
    $display("%p", q);
    wr(q[0]);
    io(q[1]);
    $display("%p", q);
    if (sum(q[0]) == 9) $display("nine");
    if (d[1].size() > 0 && d[1][0] != 2) d[1].reverse();
    sq["a"] = '{1, 2};
    sq["a"][1] = 3;
    $display("%0d %p", sq["a"][1], sq);
    r = sq["a"][0] + iq[3].size();
    sq["b"].push_back(sq["a"].pop_front());
    $display("%p", sq);
    iq[1].push_back(5);
    iq[2].push_back(iq[1].pop_front());
    $display("%0d %0d %0d", iq.num(), iq[1].size(), iq[2][0]);
    $display("%0d", sq["none"][0]);
    $finish;
  end
endmodule
