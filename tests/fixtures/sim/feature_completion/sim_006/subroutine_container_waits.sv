module tb;
  class C; int q[$]; int a[string]; C next; endclass
  C h;
  int q0[$];
  int cnt;
  task automatic loc(input int s[$]);
    int l2[$];
    l2 = s;
    fork
      #1 l2[0] = 5;
      #2 s.push_back(1);
    join_none
    wait(l2[0] == 5);
    $display("l2 %0d", $time);
    @(s.size());
    $display("s %0d", s.size());
  endtask
  task automatic g(ref int r[$]);
    wait(r.size() > 1);
    $display("r %0d", r.size());
    @(r[0]);
    $display("r0 %0d", r[0]);
  endtask
  task automatic o(output int x[$]);
    x.push_back(1);
    #1 x[0] = 2;
    wait (x[0] == 2);
    $display("x %0d", x[0]);
  endtask
  function automatic int f(C c);
    return c.q.size();
  endfunction
  task automatic hand(C c);
    wait(c.q.size() == 3);
    $display("c %0d", c.q.size());
    @(c.q[1]);
    $display("c1 %0d", c.q[1]);
  endtask
  initial begin
    h = new;
    q0.push_back(0);
    fork
      begin #1 q0.push_back(1); #1 q0[0] = 7; end
      g(q0);
    join
    begin int y[$]; o(y); $display("y %0d", y[0]); end
    h.q.push_back(0);
    fork
      begin #1 h.q.push_back(1); h.q.push_back(2); #1 h.q[1] = 8; #1 h.q.delete(2); end
      hand(h);
    join
    q0 = {1};
    loc(q0);
    fork
      #1 h.q.push_back(1);
      begin h.next = new; wait(h.next.q.size() == 0); $display("nx"); end
    join
    $finish;
  end
endmodule
