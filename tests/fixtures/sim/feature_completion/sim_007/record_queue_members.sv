// A queue of records as a member of a module record element and of a fixed array
// of records. 7.2: "A structure represents a collection of data types that can
// be referenced as a whole, or the individual data types that make up the
// structure can be referenced by name." 7.10: "A queue is a variable-size,
// ordered collection of homogeneous elements." Each element of `m.e` owns its queue.
module tb;
  typedef struct { string s; int n; } n_t;
  typedef struct { int v; n_t q[$]; } el_t;
  typedef struct { string name; el_t e[2]; } rec_t;
  rec_t m;
  el_t a[2];
  int k = 1;
  n_t x;
  initial begin
    x = '{"one", 1};
    m.e[1].q.push_back(x);
    m.e[1].q.push_back('{"two", 2});
    $display("%0d %s %0d", m.e[1].q.size(), m.e[1].q[0].s, m.e[1].q[1].n);
    m.e[1].q[0].n = 5;
    $display("%0d %0d", m.e[1].q[0].n, m.e[0].q.size());
    a[1].q.push_back(x);
    a[1].q[0].s = "alt";
    $display("%s", a[k].q[0].s);
    a[k].q[0].n = 7;
    $display("%0d", a[1].q[0].n);
    x = m.e[1].q.pop_front();
    $display("%s %0d %0d", x.s, x.n, m.e[1].q.size());
    foreach (m.e[1].q[i]) $display("%0d %s", i, m.e[1].q[i].s);
    m.e[0] = m.e[1];
    $display("%s", m.e[0].q[0].s);
    m.e[1].q[0].s = "three";
    $display("%s %s", m.e[0].q[0].s, m.e[1].q[0].s);
    a[0] = a[1];
    a[0].q[0].n = 9;
    $display("%0d %0d", a[0].q[0].n, a[1].q[0].n);
    $finish(0);
  end
endmodule
