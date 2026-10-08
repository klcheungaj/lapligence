// SIM-017: receivers and senders killed or disabled while blocked, and a
// receiver killed after a message was handed to it but before it resumed.
// No message is consumed twice, lost or written into a dead receiver's
// storage; the model then ends with queued messages, a blocked sender and a
// blocked receiver (SV 15.4, 9.6.3, 9.7).
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
        $display("unexpected consumer");
      end
    join_none
    #1;
    x.id = 4;
    x.s = "four";
    m.put(x);
    x.id = 5;
    x.s = "five";
    m.put(x);
    $display("pending n=%0d", m.num());
    k.kill();
    $display("handed back n=%0d wrote=%0d", m.num(), wrote);
    m.get(y);
    $display("head %0d %s", y.id, y.s);
    m.get(y);
    $display("next %0d %s", y.id, y.s);
    fork
      begin
        k = process::self();
        sm.peek(str);
        $display("unexpected peek");
      end
    join_none
    #1 sm.put("p");
    k.kill();
    $display("peek dropped n=%0d str=[%s]", sm.num(), str);
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
