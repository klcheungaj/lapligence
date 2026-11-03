// Waits and event controls on storage that publishes a change only once a
// wait observes it: per-object container properties, mailbox counts and
// the handles they are selected through (IEEE 1800-2009 9.4.2, 9.4.3).
module tb;
  class N;
    int q[$];
    mailbox mb = new;
  endclass
  class C;
    int q[$];
    int fa[2];
    int da[];
    int aa[string];
    N n;
    int t_own = -1;
    task automatic wait_own();
      wait (q.size() == 2);
      t_own = int'($time);
    endtask
  endclass
  class D extends C;
  endclass
  C h = new;
  C k = new;
  C g = new;
  D d = new;
  mailbox m = new;
  mailbox m2 = new;
  int t_q = -1, t_fa = -1, t_da = -1, t_aa = -1, t_q0 = -1, t_m = -1;
  int t_ev = -1, t_nq = -1, t_mb = -1, t_d = -1, t_bind = -1, t_rebind = -1;
  int t_mbind = -1, t_never = -1;
  initial h.n = new;
  initial #0 h.wait_own();
  initial begin wait (h.q.size() == 1); t_q = int'($time); end
  initial begin wait (h.fa[1] == 5); t_fa = int'($time); end
  initial begin wait (h.da.size() == 2); t_da = int'($time); end
  initial begin wait (h.aa.exists("k")); t_aa = int'($time); end
  initial begin wait (h.q.size() > 2 && h.q[2] == 7); t_q0 = int'($time); end
  initial begin wait (m.num() == 2); t_m = int'($time); end
  initial begin #0 @(h.q.size()); t_ev = int'($time); end
  initial begin #0 wait (h.n.q.size() == 1); t_nq = int'($time); end
  initial begin #0 wait (h.n.mb.num() == 1); t_mb = int'($time); end
  initial begin wait (d.q.size() == 1); t_d = int'($time); end
  initial begin wait (g.q.size() == 3); t_rebind = int'($time); end
  initial begin wait (k.q.size() == 9); t_never = int'($time); end
  initial begin
    wait (k.aa.num() == 1);
    t_bind = int'($time);
  end
  mailbox mw;
  initial begin #0 mw = m2; wait (mw.num() == 1); t_mbind = int'($time); end
  initial begin
    #1 h.q.push_back(3);
    #1 h.fa[1] = 5;
    #1 h.da = new[2];
    #1 h.aa["k"] = 1;
    #1 h.q.push_back(4);
    #1 h.q.push_back(7);
    #1 m.put(1);
    #1 m.put(2);
    #1 h.n.q.push_back(1);
    #1 h.n.mb.put(1);
    #1 d.q.push_back(4);
    #1 k.aa["x"] = 1;
    #1 k.q = '{1, 2, 3};
    #1 g = k;
    #1 m2.put(8);
    #1 m = m2;
    #1 begin
      $display("q=%0d fa=%0d da=%0d aa=%0d q2=%0d own=%0d m=%0d", t_q, t_fa, t_da, t_aa, t_q0, h.t_own, t_m);
      $display("ev=%0d nq=%0d mb=%0d d=%0d bind=%0d rebind=%0d mbind=%0d never=%0d", t_ev, t_nq, t_mb, t_d, t_bind, t_rebind, t_mbind, t_never);
    end
    $finish(0);
  end
endmodule
