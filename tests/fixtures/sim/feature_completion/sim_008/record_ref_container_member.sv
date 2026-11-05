// SV 13.5.2: record elements with queue and dynamic-array members as `ref`
// actuals. See the readme.
`timescale 1ns/1ns
module tb;
  typedef struct { int a; int q[$]; string l[$]; int d[]; } r_t;
  typedef struct { int k; r_t in; } o_t;
  typedef struct { r_t e[2]; } m_t;
  r_t list[$];
  r_t dyn[];
  r_t fix[2];
  r_t asc[int];
  r_t empty;
  o_t outer[$];
  m_t mod;
  int seen = 0;
  int i = 0;

  function automatic void f(ref r_t r);
    r.a = 1;
    r.q.push_back(7);
    r.l.push_back("x");
    r.d = new[2];
    r.d[1] = 9;
  endfunction
  function automatic int rd(const ref r_t r);
    return r.a + r.q.size() + r.l.size() + r.d.size();
  endfunction
  task automatic slow(ref r_t r);
    r.q.push_back(1);
    #2 $display("%0t slow q %0d %0d", $time, r.q.size(), r.q[r.q.size() - 1]);
    r.q.push_back(2);
    #2 $display("%0t slow done", $time);
  endtask
  task automatic twice(ref r_t a, ref r_t b);
    a.q.push_back(3);
    $display("alias %0d %0d", b.q.size(), b.q[b.q.size() - 1]);
    b.q.delete(0);
    $display("alias %0d", a.q.size());
  endtask
  task automatic moved(ref r_t r);
    #1 r.q.push_back(100);
    #1 $display("%0t moved %0d", $time, r.q.size());
  endtask
  task automatic whole(ref r_t r);
    r_t other;
    other.a = 5;
    other.q.push_back(55);
    r = other;
  endtask
  task automatic edit(ref r_t r);
    foreach (r.q[j]) r.q[j] += 1;
    r.q.sort();
    r.q.reverse();
    r.q.pop_front();
  endtask
  task automatic cancelled(ref r_t r);
    r.q.push_back(9);
    #5 r.q.push_back(10);
  endtask
  class C;
    virtual task m(ref r_t r);
      r.q.push_back(11);
    endtask
  endclass

  always @(list[1].a) seen++;

  initial begin
    C c = new;
    list.push_back(empty);
    list.push_back(empty);
    dyn = new[2];
    outer.push_back('{k: 0, in: empty});
    f(list[0]);
    $display("q %0d %0d %0d %0d %0d", list[0].a, list[0].q.size(), list[0].q[0], list[0].l.size(), list[0].d[1]);
    f(dyn[1]);
    $display("d %0d %0d %s", dyn[1].a, dyn[1].q.size(), dyn[1].l[0]);
    f(fix[i]);
    $display("f %0d %0d", fix[0].a, fix[0].q.size());
    f(asc[5]);
    $display("a %0d %0d %0d", asc[5].a, asc[5].q.size(), asc.num());
    f(outer[0].in);
    $display("m %0d %0d", outer[0].in.a, outer[0].in.q.size());
    f(mod.e[i]);
    $display("e %0d %0d", mod.e[0].a, mod.e[0].q.size());
    $display("r %0d", rd(list[0]));
    // Stores are seen at once, and stores to the element reach the formal.
    fork
      slow(list[1]);
      begin
        #1 $display("%0t main q %0d", $time, list[1].q.size());
        list[1].q.push_back(50);
      end
    join
    $display("q %0d %0d %0d", list[1].q.size(), list[1].q[1], list[1].q[2]);
    twice(list[1], list[1]);
    $display("q %0d", list[1].q.size());
    // A position moves with the element; delete outdates (7.10.3, 13.5.2).
    fork
      moved(list[1]);
      #0 list.push_front(empty);
    join
    $display("%0d %0d", list.size(), list[2].q.size());
    fork
      moved(list[0]);
      #0 list.delete(0);
    join
    $display("%0d %0d", list.size(), list[0].q.size());
    whole(list[1]);
    $display("w %0d %0d %0d", list[1].a, list[1].q.size(), list[1].q[0]);
    list[0].q = '{1, 2, 3};
    edit(list[0]);
    $display("s %0d %0d %0d", list[0].q.size(), list[0].q[0], list[0].q[1]);
    begin : block
      fork : cancel
        cancelled(list[1]);
        #1 disable cancel;
      join
    end
    #6 $display("c %0d", list[1].q.size());
    c.m(list[1]);
    $display("v %0d", list[1].q[list[1].q.size() - 1]);
    i = 9;
    f(list[i]);
    $display("x %0d", list.size());
    $display("seen %0d", seen);
    $finish;
  end
endmodule
