// SIM-017, decision AB-O3: a put that wakes a blocked receiver completes its
// get. The message leaves the queue at the put and belongs to the receiver
// from then on: a whole-variable destination (the `ref` argument of get,
// SV 15.4.5) is written at the put, and a receiver killed before it resumes
// keeps what it was delivered, so nothing returns to the mailbox. A woken
// peek copies the message at the put and leaves it queued.
module tb;
  mailbox #(int) m = new();
  mailbox #(string) sm = new();
  process k;
  int wrote;
  int got;
  int y;
  string str;

  task automatic consume(output int flag);
    int v;
    m.get(v);
    flag = v;
  endtask

  initial begin
    fork
      begin
        k = process::self();
        consume(wrote);
        $display("unexpected consumer");
      end
    join_none
    #1;
    m.put(4);
    m.put(5);
    $display("pending n=%0d", m.num());
    k.kill();
    $display("kept by receiver n=%0d wrote=%0d", m.num(), wrote);
    m.get(y);
    $display("head %0d", y);
    fork
      begin
        k = process::self();
        m.get(got);
        $display("unexpected getter");
      end
    join_none
    #1 m.put(6);
    $display("delivered got=%0d n=%0d", got, m.num());
    k.kill();
    $display("after kill got=%0d n=%0d", got, m.num());
    fork
      begin
        k = process::self();
        sm.peek(str);
        $display("unexpected peek");
      end
    join_none
    #1 sm.put("p");
    k.kill();
    $display("peek copied n=%0d str=[%s]", sm.num(), str);
    $finish(0);
  end
endmodule
