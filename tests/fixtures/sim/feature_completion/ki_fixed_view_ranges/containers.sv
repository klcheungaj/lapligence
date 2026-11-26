// Fixed arrays of queues, dynamic and associative arrays whose range does not
// start at 0, in instance and static class properties: writes, element
// writes, methods, copies, `foreach` and `%p` all address the element the
// index names.
module tb;
  class c;
    int q[1:0][$];
    static int s[1:2][$];
    static string d[2:1][];
    int a[1:2][string];

    task run();
      q[0] = '{1, 2};
      q[1].push_back(7);
      q[0][1] = 9;
      $display("q %p %0d %0d %0d", q, q[0].size(), q[1].size(), q[0][1]);
      foreach (q[i, j]) $display("q[%0d][%0d] %0d", i, j, q[i][j]);
      q[1] = q[0];
      void'(q[0].pop_front());
      $display("q %p", q);
    endtask
  endclass

  function automatic int count(int x[$]);
    return x.size();
  endfunction

  c o;

  initial begin
    o = new;
    o.run();
    c::s[1] = '{4, 5, 6};
    c::s[2].push_back(8);
    c::s[1][2] = 3;
    c::s[2].insert(0, 1);
    $display("s %p %0d %0d", c::s, count(c::s[1]), c::s[2][0]);
    foreach (c::s[i]) $display("s[%0d] %p", i, c::s[i]);
    c::d[1] = new[2];
    c::d[1][0] = "x";
    $display("d %p %0d %0d", c::d, c::d[1].size(), c::d[2].size());
    foreach (c::d[i, j]) $display("d[%0d][%0d] %s", i, j, c::d[i][j]);
    c::d[2] = c::d[1];
    c::d[1][1] = "y";
    $display("d %p", c::d);
    o.a[1]["k"] = 1;
    o.a[2]["m"] = 2;
    o.a[2]["n"] = 3;
    $display("a %p %0d %0d", o.a, o.a[1].num(), o.a[2].num());
    foreach (o.a[i, k]) $display("a[%0d][%s] %0d", i, k, o.a[i][k]);
    $finish;
  end
endmodule
