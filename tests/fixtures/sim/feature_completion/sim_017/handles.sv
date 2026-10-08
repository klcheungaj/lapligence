// SIM-017: handle messages keep the identity of their object: class handles,
// events (the synchronization object, SV 15.5.5), processes, virtual
// interfaces and mailboxes. An untyped mailbox matches the nominal type of
// the destination (SV 15.4.9, 6.22).
class obj_c;
  int v;
endclass

interface bus_if;
  logic [7:0] d;
endinterface

module tb;
  bus_if b0 ();
  bus_if b1 ();
  mailbox #(obj_c) objs = new(1);
  mailbox #(event) events = new();
  mailbox #(process) procs = new();
  mailbox #(virtual bus_if) vifs = new();
  mailbox #(mailbox #(int)) boxes = new();
  mailbox any = new();
  obj_c o, p;
  event e1, e2;
  process h, g;
  virtual bus_if v, w;
  mailbox #(int) inner, got_box;
  int x;

  initial begin
    o = new;
    o.v = 1;
    objs.put(o);
    o.v = 2;
    objs.get(p);
    $display("obj same=%0d v=%0d", p == o, p.v);
    o = new;
    o.v = 3;
    $display("rebind %0d %0d", p.v, o.v);
    events.put(e1);
    events.get(e2);
    fork
      begin
        @(e2);
        $display("e2 woke at %0d", $time);
      end
      #5 ->e1;
    join
    fork
      begin
        h = process::self();
        #10;
      end
    join_none
    #1 procs.put(h);
    procs.peek(g);
    $display("process same=%0d %s n=%0d", g == h, g.status().name(), procs.num());
    b0.d = 8'h5a;
    b1.d = 8'ha5;
    v = b0;
    vifs.put(v);
    v = b1;
    vifs.put(v);
    vifs.get(w);
    $display("vif %h", w.d);
    vifs.get(w);
    $display("vif %h", w.d);
    inner = new();
    boxes.put(inner);
    boxes.get(got_box);
    got_box.put(9);
    inner.get(x);
    $display("box same=%0d x=%0d", got_box == inner, x);
    any.put(o);
    any.put(e1);
    any.put(inner);
    $display("as_event=%0d n=%0d", any.try_get(e2), any.num());
    $display("as_obj=%0d v=%0d", any.try_get(p), p.v);
    $display("as_box=%0d", any.try_get(got_box));
    $display("as_event=%0d", any.try_get(e2));
    $display("as_box=%0d same=%0d n=%0d", any.try_get(got_box), got_box == inner, any.num());
    $finish;
  end
endmodule
