module tb;
  class C; int q[$]; string s[$]; C next; endclass
  typedef struct { int q[$]; int v; } rec_t;
  rec_t r;
  int q[$];
  int d[];
  function automatic void addq(ref int t[$], input int v);
    t.push_back(v);
  endfunction
  function automatic void addd(ref int t[], input int n);
    t = new[n];
  endfunction
  function automatic void bump(ref int t[$]);
    addq(t, t.size());
  endfunction
  task automatic late(ref int t[$]);
    #1 t.push_back(99);
  endtask
  C h, l[2];
  initial begin
    h = new; l[1] = new;
    addq(q, 1); bump(q);
    $display("%0d %0d", q.size(), q[1]);
    addq(h.q, 2); bump(h.q);
    $display("%0d %0d", h.q.size(), h.q[1]);
    addq(l[1].q, 3);
    $display("%0d", l[1].q[0]);
    addq(r.q, 4);
    $display("%0d", r.q[0]);
    fork late(q); late(h.q); join
    $display("%0d %0d", q[2], h.q[2]);
    addd(d, 3);
    $display("%0d", d.size());
    $finish;
  end
endmodule
