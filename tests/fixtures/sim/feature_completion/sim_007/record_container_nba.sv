// Nonblocking writes of records with container members and of whole
// container members.
module tb;
  typedef struct { string s; int q[$]; int d[]; int a[string]; string sq[$]; } rec_t;
  rec_t m, n, o;
  typedef struct { string s; int q[$]; string sq[$]; } erc_t;
  erc_t el[2];
  erc_t e1;
  int qv[$] = '{7, 8};
  initial begin
    m.s = "m"; m.q.push_back(1); m.d = new[2]; m.d[1] = 5; m.a["x"] = 3; m.sq.push_back("z");
    n <= m;
    m.q.push_back(2);
    m.a["y"] = 4;
    $display("%0d", n.q.size());
    #1 $display("[%s] %0d %0d %0d %0d %s", n.s, n.q.size(), n.d[1], n.a.size(), n.a["x"], n.sq[0]);
    o <= #2 n;
    n.q.push_back(9);
    #1 $display("%0d", o.q.size());
    #1;
    #1 $display("%0d %0d", o.q.size(), o.a.size());
    // element container members
    e1.s = "e"; e1.q = m.q;
    el[1] <= e1;
    #1 $display("[%s] %0d", el[1].s, el[1].q.size());
    el[0].q <= qv;
    el[0].sq <= '{"a", "b"};
    qv.push_back(1);
    $display("%0d", el[0].q.size());
    #1 $display("%0d %0d %s", el[0].q.size(), el[0].q[1], el[0].sq[1]);
    el[0].q <= #1 '{1};
    el[0] = e1;
    #2 $display("%0d %0d", el[0].q.size(), el[0].q[0]);
    $finish(0);
  end
endmodule
