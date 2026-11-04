// SV 13.5.2, 7.10.3: real elements of resizable arrays and real members of
// records as `ref` actuals. See the readme.
`timescale 1ns/1ns
module tb;
  typedef struct { string s; real r; real a[2]; shortreal h; } rec_t;
  real q[$];
  real d[];
  real aa[int];
  real sa[string];
  shortreal sq[$];
  rec_t ar[2];
  rec_t rq[$];
  rec_t mr;
  real seen;
  int k = 1;
  task automatic set(ref real x, input real v); x = v; endtask
  task automatic setsh(ref shortreal x, input shortreal v); x = v; endtask
  function automatic real rd(const ref real x); return x + 1.0; endfunction
  function automatic real bump(ref real x); x = x + 1.0; return x; endfunction
  task automatic hold(ref real x);
    $display("%0t hold in %f", $time, x);
    #2 $display("%0t hold sees %f", $time, x);
    x = 9.5;
    #2 $display("%0t hold end %f", $time, x);
  endtask
  task automatic waiter(ref real x);
    @(x) $display("%0t waiter %f", $time, x);
  endtask
  always_comb seen = q.size() > 1 ? q[1] : 0.0;
  initial begin
    q.push_back(1.25); q.push_back(2.25);
    set(q[0], 2.5);
    $display("q %f %f", q[0], q[1]);
    d = new[2];
    set(d[k], 3.5);
    $display("d %f %f", d[0], d[1]);
    set(aa[7], 4.5);
    $display("aa %0d %f", aa.size(), aa[7]);
    set(sa["x"], 5.5);
    $display("sa %f", sa["x"]);
    sq.push_back(0.5);
    setsh(sq[0], 1.5);
    $display("sq %f", sq[0]);
    $display("rd %f %f", rd(q[0]), bump(q[1]));
    $display("q %f %f", q[0], q[1]);
    set(ar[1].r, 6.5);
    set(ar[1].a[k], 7.5);
    setsh(ar[1].h, 8.5);
    $display("ar %f %f %f", ar[1].r, ar[1].a[1], ar[1].h);
    rq.push_back(ar[1]);
    set(rq[0].r, 1.5);
    $display("rq %f", rq[0].r);
    set(mr.a[k], 2.5);
    $display("mr %f", mr.a[1]);
    fork
      hold(q[0]);
      begin #1 q.push_front(0.25); #2 $display("%0t q %f %f %f", $time, q[0], q[1], q[2]); end
      begin #1 q[1] = 8.5; end
      waiter(q[1]);
      begin #2 q[2] = 11.5; end
    join
    #1 $display("seen %f", seen);
    // A removed element leaves the formal its last value, private.
    fork
      begin q.delete(1); end
    join
    fork
      hold(d[0]);
      begin #1 d.delete(); end
    join
    $display("d %0d", d.size());
    $finish;
  end
endmodule
