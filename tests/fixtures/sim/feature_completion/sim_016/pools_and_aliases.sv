// SIM-016: semaphores in an array keep separate key pools; copies of one
// handle (a variable, a queue element and task arguments) share its pool,
// and blocked gets are served first in, first out (SV 15.3).
module tb;
  semaphore pool [3];
  semaphore q[$];

  task automatic take(semaphore s, int id);
    s.get(1);
    $display("take %0d %0d", id, $time);
  endtask

  initial begin
    semaphore a, b;
    foreach (pool[i]) pool[i] = new(i);
    if (!pool[0].try_get(1)) $display("pool0 empty");
    if (pool[1].try_get(1) && !pool[1].try_get(1)) $display("pool1 one key");
    if (pool[2].try_get(2)) $display("pool2 two keys");
    a = new(0);
    b = a;
    q.push_back(a);
    fork
      take(a, 1);
      begin #1 take(b, 2); end
      begin #2 take(q[0], 3); end
    join_none
    #3 b.put(1);
    #1 q[0].put(1);
    #1 a.put(1);
    #1 $finish;
  end
endmodule
