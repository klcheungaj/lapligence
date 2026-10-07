// SIM-017: mailbox handles stored in record members, class properties,
// fixed, dynamic and associative arrays and queues, and passed to task
// formals by value and by ref. Every copy names the same mailbox
// (SV 15.4, 8.4, 7.2, 13.5).
class agent_c;
  mailbox #(int) inbox;
  mailbox any;

  function new();
    inbox = new(1);
  endfunction

  task automatic send(int value);
    inbox.put(value);
  endtask
endclass

module tb;
  typedef struct {
    mailbox #(int) box;
    int tag;
  } port_t;

  port_t p, p_copy;
  agent_c agent;
  mailbox #(int) fixed_a[2];
  mailbox #(int) dyn_a[];
  mailbox #(int) assoc_a[string];
  mailbox #(int) queue_a[$];
  mailbox #(int) made;
  int v, w;

  task automatic put_by_value(mailbox #(int) box, input int value);
    box.put(value);
  endtask

  task automatic make_by_ref(ref mailbox #(int) box, input int value);
    box = new();
    box.put(value);
  endtask

  initial begin
    p.box = new();
    p.tag = 1;
    p_copy = p;
    p.box.put(5);
    p_copy.box.get(v);
    $display("record %0d n=%0d", v, p.box.num());
    agent = new();
    agent.send(6);
    $display("class full=%0d", agent.inbox.try_put(7));
    agent.inbox.get(v);
    agent.any = new();
    agent.any.put(8);
    agent.any.get(w);
    $display("class %0d %0d", v, w);
    fixed_a[0] = new();
    fixed_a[1] = fixed_a[0];
    dyn_a = new[1];
    dyn_a[0] = fixed_a[1];
    assoc_a["k"] = dyn_a[0];
    queue_a.push_back(assoc_a["k"]);
    queue_a[0].put(9);
    fixed_a[0].peek(v);
    put_by_value(dyn_a[0], 10);
    $display("arrays %0d n=%0d", v, assoc_a["k"].num());
    fixed_a[1].get(v);
    fixed_a[1].get(w);
    $display("arrays %0d %0d", v, w);
    make_by_ref(made, 11);
    made.get(v);
    $display("ref %0d null=%0d", v, made == null);
    $finish;
  end
endmodule
