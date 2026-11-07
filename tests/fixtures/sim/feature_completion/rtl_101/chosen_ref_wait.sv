// Tasks waiting on `ref` formals bound to elements selected at run time, in
// records with members held in containers: each wakes for its element only
// (SV 9.4.2, 13.5.2), through the element, its record or another reference.
`timescale 1ns/1ns
`ifndef N
`define N 16
`endif
module tb;
  typedef struct {
    string s;
    int n;
    bit b;
    int q[$];
    int a[`N];
    string t[`N];
    real d[`N];
  } el_t;
  typedef struct { el_t e[`N]; string name; int k; } rec_t;
  rec_t o;
  el_t mv, xx;
  el_t dy[];
  el_t qu[$];
  el_t as[int];
  int i = 1, j = 2;

  task automatic wn(ref el_t r, input string tag);
    @(r.n);
    $display("%0t %s n %0d", $time, tag, r.n);
  endtask
  task automatic wb(ref el_t r);
    @(posedge r.b);
    $display("%0t wb %0d", $time, r.b);
  endtask
  task automatic wa(ref el_t r, input int k);
    @(r.a[k]);
    $display("%0t wa %0d", $time, r.a[k]);
  endtask
  task automatic wd(ref el_t r, input int k);
    @(r.d[k]);
    $display("%0t wd %0.2f", $time, r.d[k]);
  endtask
  task automatic ws(ref el_t r);
    @(r.s);
    $display("%0t ws [%s]", $time, r.s);
  endtask
  task automatic wq(ref el_t r);
    wait (r.q.size() > 1);
    $display("%0t wq %0d", $time, r.q.size());
  endtask
  task automatic wt(ref el_t r, input int k);
    @(r.t[k]);
    $display("%0t wt [%s]", $time, r.t[k]);
  endtask
  task automatic wc(ref el_t r);
    wait (r.n == 42);
    $display("%0t wc", $time);
  endtask
  task automatic fw(ref el_t r, input string tag);
    #0 wn(r, tag);
  endtask
  task automatic loop3(ref el_t r, input int c);
    @(r.n);
    $display("%0t loop %0d %0d", $time, c, r.n);
    if (c > 1) loop3(r, c - 1);
  endtask
  task automatic two(ref el_t a, ref el_t b);
    @(a.n or b.n);
    $display("%0t two %0d %0d", $time, a.n, b.n);
  endtask
  task automatic wr(ref el_t r, input int v);
    #1 r.n = v;
  endtask
  task automatic inner(ref el_t r);
    fork
      begin
        @(r.n);
        $display("%0t br1 %0d", $time, r.n);
      end
      begin
        @(r.a[0]);
        $display("%0t br2 %0d", $time, r.a[0]);
      end
    join
  endtask
  task automatic self(ref el_t r);
    r.n = 1;
    @(r.n);
    $display("%0t self %0d", $time, r.n);
    r.n = 2;
    #1 $display("%0t self2 %0d", $time, r.n);
  endtask
  class C;
    virtual task m(ref el_t r);
      @(r.n);
      $display("%0t C %0d", $time, r.n);
    endtask
  endclass
  class D extends C;
    virtual task m(ref el_t r);
      r.n = 5;
      @(r.n);
      $display("%0t D %0d", $time, r.n);
    endtask
  endclass
  C c = new;
  D d = new;
  C b;

  initial begin
    // writes of the element wake; writes of other elements do not
    fork
      wn(o.e[i], "a");
      begin
        #1 o.e[0].n = 100;
        o.e[2].n = 200;
        o.name = "n";
        #1 o.e[1].n = 101;
      end
    join
    fork
      wb(o.e[i]);
      #1 o.e[1].b = 1;
    join
    fork
      wa(o.e[i], 3);
      begin
        #1 o.e[1].a[2] = 5;
        #1 o.e[1].a[3] = 6;
      end
    join
    fork
      wd(o.e[i], 2);
      begin
        #1 o.e[1].d[1] = 1.5;
        #1 o.e[1].d[2] = 2.5;
      end
    join
    fork
      ws(o.e[i]);
      begin
        #1 o.e[0].s = "other";
        #1 o.e[1].s = "s1";
      end
    join
    fork
      wq(o.e[i]);
      begin
        #1 o.e[1].q.push_back(1);
        #1 o.e[1].q.push_back(2);
      end
    join
    fork
      wt(o.e[i], 2);
      begin
        #1 o.e[1].t[1] = "t1";
        #1 o.e[1].t[2] = "t2";
      end
    join
    fork
      wc(o.e[i]);
      begin
        #1 o.e[1].n = 41;
        #1 o.e[1].n = 42;
      end
    join
    fork
      fw(o.e[i], "f");
      #1 o.e[1].n = 43;
    join
    fork
      loop3(o.e[i], 2);
      begin
        #1 o.e[1].n = 44;
        #1 o.e[1].n = 45;
      end
    join
    $display("-- whole");
    // a whole element, a nonblocking write, a store through another reference
    fork
      wn(o.e[j], "w");
      begin
        #1 xx.n = 7;
        o.e[j] = xx;
      end
    join
    fork
      wn(o.e[j], "x");
      #1 o.e[j].n <= 8;
    join
    fork
      wn(o.e[j], "y");
      wr(o.e[j], 9);
    join
    $display("-- two");
    fork
      two(o.e[1], o.e[2]);
      #1 o.e[2].n = 11;
    join
    fork
      two(o.e[i], mv);
      #1 mv.n = 12;
    join
    fork
      inner(o.e[1]);
      begin
        #1 o.e[1].n = 13;
        #1 o.e[1].a[0] = 14;
      end
    join
    fork
      self(o.e[1]);
      #1 o.e[1].n = 15;
    join
    $display("-- virtual");
    fork
      c.m(o.e[i]);
      #1 o.e[1].n = 16;
    join
    b = d;
    fork
      b.m(o.e[i]);
      #1 o.e[1].n = 17;
    join
    $display("-- others");
    dy = new[3];
    qu = {xx, xx};
    as[5] = xx;
    fork
      wn(dy[i], "dy");
      begin
        #1 dy[0].n = 1;
        #1 dy[1].n = 21;
      end
    join
    fork
      wn(qu[i], "qu");
      begin
        #1 qu[0].n = 1;
        #1 qu[1].n = 22;
      end
    join
    fork
      wn(as[5], "as");
      #1 as[5].n = 23;
    join
    // the wait ends with its process
    fork : guard
      wn(o.e[1], "gone");
      #1 disable guard;
    join
    #1 o.e[1].n = 77;
    $display("%0t end", $time);
    $finish(0);
  end
endmodule
