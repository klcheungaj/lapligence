// Static properties that are fixed arrays of queues, dynamic and associative
// arrays and events.
module tb;
  typedef struct {
    int    i;
    string s;
  } rec_t;

  class c #(int W = 1);
    static int q[2][$];
    static string d[2][];
    static int a[2][string];
    static rec_t r[2][$];
    static int n[2][$][$];
    static event e[2];
    static c #(W) h[2][$];

    static function void fill(int v);
      q[1].push_back(v);
      d[0] = new[2];
      d[0][1] = $sformatf("s%0d", v);
      a[1]["k"] = v + 1;
      r[0].push_back('{v + 2, "r"});
      n[1].push_back('{v, v + 3});
    endfunction

    function int size();
      return q[0].size() + q[1].size();
    endfunction
  endclass

  c #(1) o = new;
  c #(2) none;

  initial begin
    c #(1)::fill(5);
    none.fill(7);
    o.q[0] = '{8, 9};
    none.q[1].push_front(4);
    $display("%0d %0d %0d %0d", c #(1)::q[1][0], o.q[0].size(), o.size(), c #(2)::q[1][0]);
    $display("%s %0d %s", o.d[0][1], c #(1)::d[1].size(), none.d[0][1]);
    $display("%0d %0d %0d", c #(1)::a[1]["k"], o.a[0].num(), none.a[1]["k"]);
    $display("%0d %s %0d", o.r[0][0].i, c #(1)::r[0][0].s, none.r[1].size());
    $display("%0d %0d", o.n[1][0][1], none.n[1][0][1]);
    c #(1)::h[1].push_back(o);
    o.h[1].push_back(null);
    $display("%b %b %0d", c #(1)::h[1][0] == o, o.h[1][1] == null, o.h[0].size());
    foreach (c #(1)::q[i]) $display("%0d: %p", i, c #(1)::q[i]);
    fork
      @(c #(1)::e[1]) $display("e %0t", $time);
      @(none.e[0]) $display("none %0t", $time);
    join_none
    #1 -> o.e[1];
    c #(2)::e[0] = c #(1)::e[0];
    #1 -> c #(1)::e[0];
    #1 $display("%b", none.e[0] == o.e[0]);
    $finish;
  end
endmodule
