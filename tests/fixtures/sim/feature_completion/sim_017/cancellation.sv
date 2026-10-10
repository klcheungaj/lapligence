// SIM-017: receivers and senders killed or disabled while blocked, and a
// receiver killed after a put woke it. No message is consumed twice or lost,
// and a blocked receiver's storage is never written; the model then ends with
// queued messages, a blocked sender and a blocked receiver (SV 15.4, 9.6.3,
// 9.7). A woken receiver is no longer blocked, so 9.7 lets it be terminated
// "at some unspecified time in the current time step": it may or may not take
// its message first, and only conservation is printed for it. llg's order is
// pinned by `policy_woken_kill`.
module tb;
  typedef struct {
    int id;
    string s;
    int q[$];
  } msg_t;

  mailbox #(msg_t) m = new(2);
  mailbox #(string) sm = new();
  msg_t x, y, keep;
  process g, p, k;
  string str;
  int wrote;
  int ids[$];
  int pending;

  task automatic consume(output int flag);
    msg_t local_m;
    m.get(local_m);
    flag = local_m.id;
  endtask

  initial begin
    keep.id = -1;
    keep.s = "untouched";
    fork
      begin
        g = process::self();
        m.get(keep);
      end
    join_none
    #1 g.kill();
    x.id = 1;
    x.s = "one";
    x.q = '{1};
    m.put(x);
    $display("killed getter n=%0d keep=%0d %s", m.num(), keep.id, keep.s);
    x.id = 2;
    x.s = "two";
    m.put(x);
    fork
      begin
        p = process::self();
        x.id = 3;
        m.put(x);
      end
    join_none
    #1 p.kill();
    $display("killed putter n=%0d", m.num());
    m.get(y);
    m.get(y);
    $display("fifo %0d %s n=%0d", y.id, y.s, m.num());
    fork
      begin
        k = process::self();
        consume(wrote);
      end
    join_none
    #1;
    x.id = 4;
    x.s = "four";
    m.put(x);
    x.id = 5;
    x.s = "five";
    m.put(x);
    k.kill();
    #1;
    pending = m.num();
    repeat (pending) begin
      m.get(y);
      ids.push_back(y.id);
    end
    if (wrote != 0) ids.push_back(wrote);
    ids.sort();
    $display("woken kill conserves %0d: %0d %0d", ids.size(), ids[0], ids[1]);
    fork
      begin
        k = process::self();
        sm.peek(str);
      end
    join_none
    #1 sm.put("p");
    k.kill();
    #1 $display("peek kept n=%0d", sm.num());
    str = "";
    wrote = 0;
    fork
      consume(wrote);
    join_none
    #1 disable fork;
    x.id = 6;
    m.put(x);
    $display("disabled n=%0d wrote=%0d", m.num(), wrote);
    x.id = 7;
    m.put(x);
    fork
      begin
        x.id = 8;
        m.put(x);
      end
      begin
        sm.get(str);
        sm.get(str);
      end
    join_none
    #1 $display("teardown n=%0d %0d str=%s", m.num(), sm.num(), str);
    $finish;
  end
endmodule
