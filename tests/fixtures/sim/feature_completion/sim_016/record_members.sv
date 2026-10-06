// SIM-016: a semaphore member of a record holds a handle: module, block and
// task-local records, record array elements and record formals copy the
// handle, so every copy uses the same key pool (SV 15.3, 7.2).
typedef struct { semaphore lock; string tag; } rec_t;

module tb;
  rec_t r;
  rec_t arr[2];

  task automatic take(input rec_t x);
    x.lock.get(1);
    $display("%s took %0d", x.tag, $time);
  endtask

  task automatic local_rec();
    rec_t l;
    l.lock = new(0);
    fork #2 l.lock.put(2); join_none
    l.lock.get(2);
    $display("local got %0d try %0d", $time, l.lock.try_get(1));
  endtask

  initial begin
    automatic rec_t b;
    r.lock = new(1);
    r.tag = "r";
    fork
      begin r.lock.get(1); $display("A got %0d", $time); #2 r.lock.put(1); end
      begin #1 r.lock.get(1); $display("B got %0d", $time); r.lock.put(1); end
    join
    $display("try %0d null %0d", r.lock.try_get(1), r.lock == null);
    b.lock = new(0);
    b.tag = "b";
    arr[0] = b;
    arr[0].tag = "a0";
    fork
      take(arr[0]);
      begin #3 b.lock.put(1); end
    join
    local_rec();
    $finish;
  end
endmodule
