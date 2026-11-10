module tb;
  int sq[string][$];
  int qq[$][$];
  string a = "a", b = "b";
  int calls;
  bit c;
  int n;
  function automatic int f();
    calls++;
    return 0;
  endfunction
  function automatic string g();
    calls++;
    return "a";
  endfunction
  initial begin
    sq["a"] = '{1, 2};
    sq[b].push_back(sq[a].pop_front());
    $display("a %p %p", sq["a"], sq["b"]);
    b = "a";
    sq[b].push_back(sq[a].pop_front());
    $display("b %p", sq["a"]);
    sq[g()].push_back(5);
    $display("c calls %0d %p", calls, sq["a"]);
    qq.push_back({});
    qq[0].push_back(4);
    if (c && qq[f()].sum() > 0) n = 1;
    $display("d calls %0d n %0d", calls, n);
    c = 1'b1;
    if (c && qq[f()].sum() > 0) n = 2;
    $display("e calls %0d n %0d", calls, n);
    n = c ? qq[f()].sum() : sq[g()].sum();
    $display("f calls %0d n %0d", calls, n);
    n = qq[f()].pop_front() + qq[f()].size();
    $display("g calls %0d n %0d", calls, n);
    $display("done");
    $finish;
  end
endmodule
