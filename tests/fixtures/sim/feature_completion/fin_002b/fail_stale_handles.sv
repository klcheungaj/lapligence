// FIN-002 step 4, stale handles: every handle is used after the thing it
// names is gone. A finished and a killed process (status, kill, await), a
// closed file descriptor (write, read, close again, end-of-file test), a
// deleted queue element and associative entry, a class object reachable
// only through a second handle after the first is cleared, an event after
// its last waiter was killed, and a mailbox emptied under a try_get.
`timescale 1ns/1ns
module tb;
  class node;
    int value;
    node next;
    function new(int v); value = v; endfunction
  endclass

  process done, killed;
  integer fd, code;
  int q [$];
  int map [string];
  node first, second;
  event e;
  mailbox #(int) box = new();
  string line;
  int v;

  initial begin
    fork
      begin done = process::self(); #1; end
      begin killed = process::self(); @e; $display("never printed"); end
    join_none
    #2;
    killed.kill();
    $display("done=%s killed=%s", done.status().name(), killed.status().name());
    done.kill();
    killed.kill();
    done.await();
    killed.await();
    $display("after kill and await: done=%s killed=%s", done.status().name(), killed.status().name());
    -> e;
    #1;

    fd = $fopen("stale_handles.txt", "w");
    $fdisplay(fd, "one line");
    $fclose(fd);
    $fdisplay(fd, "after close");
    $fclose(fd);
    fd = $fopen("stale_handles.txt", "r");
    code = $fgets(line, fd);
    $display("first read %0d", code);
    code = $fgets(line, fd);
    $display("second read %0d eof=%0d", code, $feof(fd) != 0);
    $fclose(fd);
    code = $fgets(line, fd);
    $display("read after close %0d", code);
    code = $fgetc(fd);
    $display("getc after close %0d", code);

    q = '{1, 2, 3};
    q.delete(1);
    $display("queue size %0d last %0d", q.size(), q[$]);
    q.delete();
    $display("queue size %0d pop %0d", q.size(), q.size() ? q.pop_front() : -1);
    map["a"] = 5;
    map.delete("a");
    $display("map exists %0d num %0d", map.exists("a"), map.num());

    first = new(1);
    first.next = new(2);
    second = first.next;
    first = null;
    $display("second %0d first is null %0d", second.value, first == null);
    second.next = second;
    second = null;

    box.put(3);
    $display("try_get %0d %0d", box.try_get(v), v);
    $display("try_get %0d num %0d", box.try_get(v), box.num());
    $finish;
  end
endmodule
