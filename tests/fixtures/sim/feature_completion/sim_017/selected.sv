// SIM-017: blocked get/peek into selected and automatic destinations. The
// message argument is a ref (Annex G.4): its selectors are fixed when the
// call starts, so changing the index while the receiver waits does not move
// the write (SV 13.5.2, 15.4.5, 15.4.7).
module tb;
  typedef struct {
    int f;
    string g;
  } r_t;

  mailbox #(int) ma = new();
  mailbox #(int) mr = new();
  mailbox #(int) mq = new();
  mailbox #(int) ml = new();
  mailbox #(r_t) rm = new();
  int a[4];
  int q[$];
  r_t r;
  r_t rq[$];
  int idx;
  int out;

  task automatic receive_local(output int result);
    int loc;
    ml.get(loc);
    result = loc * 10;
  endtask

  initial begin
    q = '{0, 0};
    rq = '{'{1, "a"}, '{2, "b"}};
    fork
      ma.get(a[idx]);
      mr.get(r.f);
      begin
        mq.peek(q[$]);
        mq.get(q[0]);
      end
      receive_local(out);
      rm.get(rq[idx]);
    join_none
    #1 idx = 1;
    ma.put(11);
    mr.put(22);
    mq.put(33);
    ml.put(33);
    rm.put('{7, "seven"});
    #1;
    $display("a %0d %0d %0d", a[0], a[1], a[2]);
    $display("r.f %0d", r.f);
    $display("q %0d %0d", q[0], q[1]);
    $display("local %0d", out);
    $display("rq %0d %s %0d %s", rq[0].f, rq[0].g, rq[1].f, rq[1].g);
    $display("n=%0d %0d %0d %0d %0d", ma.num(), mr.num(), mq.num(), ml.num(), rm.num());
    $finish;
  end
endmodule
