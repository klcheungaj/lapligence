// SIM-017 llg policy, not an IEEE 1800-2009 requirement. 9.7: "If the
// process to be terminated is not blocked waiting on some other condition,
// such as an event, wait expression, or a delay, then the process shall be
// terminated at some unspecified time in the current time step." A receiver
// woken by a put is no longer blocked, so killing it may happen before or
// after it takes its message. llg hands a message to a blocked getter at the
// put (it leaves the queue then), and a kill before the getter resumes puts
// the message back at the head of the queue; a woken peek waiter killed
// before it resumes copies nothing. The portable oracle (conservation only)
// is `cancellation`.
module tb;
  mailbox #(int) m = new();
  mailbox #(string) sm = new();
  process k;
  int wrote;
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
    $display("handed back n=%0d wrote=%0d", m.num(), wrote);
    m.get(y);
    $display("head %0d", y);
    m.get(y);
    $display("next %0d", y);
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
    $finish(0);
  end
endmodule
