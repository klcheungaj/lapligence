// Fixed arrays of queues, dynamic and associative arrays declared in a
// module and in automatic and static subroutine storage are fixed-size views
// of the container runtime, like class properties.
module tb;
  int q[1:0][$];
  string d[1:2][];
  int m[2:1][int];

  function automatic int f(int n);
    int aq[1:0][$];
    string sd[2][];
    aq[1] = '{1};
    aq[0] = '{2, 3};
    aq[0].push_back(n);
    sd[1] = new[2];
    sd[1][0] = "s";
    $display("f %p %p", aq, sd);
    return aq[0].size();
  endfunction

  function int g(int n);
    static int sq[2][$];
    sq[n % 2].push_back(n);
    return sq[0].size() * 10 + sq[1].size();
  endfunction

  initial begin
    q[0] = '{1, 2};
    q[1].push_back(7);
    q[0][1] = 9;
    $display("q %p %0d %0d", q, q[0].size(), q[1][0]);
    foreach (q[i, j]) $display("q[%0d][%0d] %0d", i, j, q[i][j]);
    d[2] = new[1];
    d[2][0] = "x";
    d[1] = d[2];
    $display("d %p", d);
    $display("%0d %0d", f(3), f(4));
    $display("%0d %0d %0d", g(1), g(2), g(4));
    m[2][5] = 1;
    m[1][-1] = 2;
    foreach (m[i, k]) $display("m[%0d][%0d] %0d", i, k, m[i][k]);
    $finish;
  end
endmodule
