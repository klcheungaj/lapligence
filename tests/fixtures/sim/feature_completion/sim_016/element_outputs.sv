// SIM-016: output and inout semaphore formals copy out into fixed-array,
// queue and string-keyed associative elements selected when the call starts
// (SV 13.3, 15.3); `ref` semaphore formals alias handle variables (13.5.2).
module tb;
  semaphore pool [2];
  semaphore q[$];
  semaphore m[string];

  task automatic make(output semaphore s, input int k);
    s = new(k);
  endtask

  task automatic make_later(output semaphore s, input int k);
    #1 s = new(k);
  endtask

  task automatic swap(inout semaphore s);
    if (!s.try_get(1)) $display("inout saw drained");
    s = new(5);
  endtask

  task automatic use_ref(ref semaphore s, input int id);
    s.get(1);
    $display("ref %0d %0d", id, $time);
  endtask

  initial begin
    semaphore t, r;
    make(pool[1], 2);
    if (pool[1].try_get(2)) $display("fixed element");
    q.push_back(t);
    make_later(q[0], 3);
    if (q[0].try_get(3)) $display("queue element %0d", $time);
    make_later(m["k"], 4);
    if (m["k"].try_get(4)) $display("assoc element %0d", $time);
    swap(m["k"]);
    if (m["k"].try_get(5)) $display("swapped");
    r = new(0);
    fork use_ref(r, 1); join_none
    #1 r.put(1);
    #1 $finish;
  end
endmodule
