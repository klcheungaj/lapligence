// Class and built-in handle elements of fixed and resizable arrays as `ref`
// actuals (SV 13.5.2, 7.10.3, 15.3, 15.4). See the readme.
module tb;
  class C;
    int v;
    function new(int x);
      v = x;
    endfunction
  endclass

  semaphore sa[2];
  semaphore sq[$];
  semaphore sd[];
  semaphore sm[int];
  mailbox #(int) ma[2];
  mailbox #(int) mq[$];
  C ca[2];
  C cd[];
  C cq[$];
  C cm[string];
  process pa[2];
  chandle ch[2];

  task automatic take(ref semaphore s, input int n);
    s.get(n);
    $display("took %0d", n);
  endtask
  task automatic put(ref mailbox #(int) m, input int v);
    m.put(v);
  endtask
  task automatic get(ref mailbox #(int) m);
    int v;
    m.get(v);
    $display("got %0d", v);
  endtask
  task automatic rebind(ref semaphore s);
    semaphore o = s;
    s = new(9);
    $display("kept %0d", o != null);
  endtask
  task automatic setc(ref C c, input int v);
    c = new(v);
  endtask
  function automatic int getv(const ref C c);
    return c.v;
  endfunction
  function automatic void bump(ref C c);
    c.v++;
  endfunction
  function automatic C pick(ref C c);
    return c;
  endfunction
  task automatic self_proc(ref process p);
    p = process::self();
    #1 $display("self %0d", p != null);
  endtask
  task automatic nullc(ref chandle c);
    $display("chandle %0d", c == null);
  endtask

  task automatic shifted(ref C x);
    C t;
    x = new(1);
    t = new(9);
    cq.push_front(t);
    $display("D1 x=%0d q0=%0d q1=%0d sz=%0d", x.v, cq[0].v, cq[1].v, cq.size());
    x = new(2);
    $display("D2 x=%0d q0=%0d q1=%0d", x.v, cq[0].v, cq[1].v);
    t = new(5);
    cq[1] = t;
    $display("D3 x=%0d", x.v);
    cq.delete(1);
    $display("D4 x=%0d sz=%0d", x.v, cq.size());
    x = new(3);
    $display("D5 x=%0d sz=%0d q0=%0d", x.v, cq.size(), cq[0].v);
  endtask

  task automatic late(ref C x, input int v);
    #1 x = new(v);
    #1 $display("late %0d", x.v);
  endtask

  initial begin
    int i = 1;
    semaphore t;
    mailbox #(int) m;
    C c;
    sa[0] = new(3);
    sa[1] = new(0);
    take(sa[0], 3);
    t = new(2);
    sq.push_back(t);
    take(sq[0], 2);
    sd = new[2];
    sd[1] = new(1);
    take(sd[i], 1);
    sm[4] = new(4);
    take(sm[4], 4);
    fork
      take(sa[i], 1);
      #1 sa[1].put(1);
    join
    ma[0] = new;
    ma[1] = new;
    put(ma[1], 5);
    get(ma[1]);
    m = new;
    mq.push_back(m);
    put(mq[0], 6);
    get(mq[0]);
    rebind(sa[0]);
    take(sa[0], 9);
    setc(ca[1], 3);
    $display("c %0d", ca[1].v);
    cd = new[2];
    setc(cd[i], 7);
    $display("cd %0d", cd[1].v);
    setc(cm["k"], 8);
    $display("cm %0d %0d", cm.num(), cm["k"].v);
    bump(ca[1]);
    $display("%0d %0d", getv(ca[1]), pick(ca[1]).v);
    c = new(7);
    cq.push_back(c);
    c = new(8);
    cq.push_back(c);
    shifted(cq[0]);
    fork
      late(ca[i], 11);
      #1.5 $display("ca %0d", ca[1].v);
    join
    self_proc(pa[1]);
    $display("pa %0d", pa[1] != null);
    nullc(ch[1]);
    $finish;
  end
endmodule
