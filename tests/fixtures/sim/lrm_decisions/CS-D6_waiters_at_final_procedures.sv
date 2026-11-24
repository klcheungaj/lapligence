// Decision CS-D6: final procedures run in the state the run ended in. The
// processes of the run still exist and never run again: a blocked process is
// WAITING (also one that was woken but did not run yet), the process that
// called $finish is RUNNING, and kill() works. A process blocked in a mailbox
// get or put or a semaphore get keeps its place: it takes the message (its
// argument is written) or the keys a final procedure supplies, and its message
// enters a mailbox a final procedure empties. An event trigger or a write in a
// final procedure wakes no process and evaluates no process's event condition,
// and a force of the run stays in effect.
//
// IEEE 1800-2009 9.2.3 (SystemVerilog-1800-2009.txt L11261): "A final
// procedure executes when simulation ends due to an explicit or implicit call
// to $finish." The text does not say what state a process that was blocked at
// that point has.
// IEEE 1800-2009 9.7 (L12616-12617): "RUNNING means the process is currently
// running (not in a blocking statement)." / "WAITING means the process is
// waiting in a blocking statement."
module tb;
  class payload;
    int v;
  endclass
  process p_event, p_level, p_time, p_get, p_put, p_keys, p_join, p_done, p_killed, p_await;
  process p_ready, p_finish;
  mailbox #(int) box = new();
  mailbox #(int) full = new(1);
  mailbox #(string) names = new();
  mailbox #(payload) objects = new();
  semaphore keys = new(1);
  payload item = new, taken;
  event go, gated;
  string name;
  int sig, got, forced, conditions, v;

  function int condition();
    conditions++;
    return 1;
  endfunction

  function string st(process p);
    return p.status().name();
  endfunction

  initial begin p_event = process::self(); @go; $display("not reached"); end
  initial begin @(gated iff condition()); $display("not reached"); end
  initial begin p_level = process::self(); wait (sig == 5); $display("not reached"); end
  initial begin p_time = process::self(); #1000 $display("not reached"); end
  initial begin p_get = process::self(); box.get(got); $display("not reached"); end
  initial begin p_put = process::self(); full.put(1); full.put(2); $display("not reached"); end
  initial begin p_keys = process::self(); keys.get(3); $display("not reached"); end
  initial begin p_join = process::self(); fork #500; join $display("not reached"); end
  initial begin p_done = process::self(); #1; end
  initial begin p_killed = process::self(); #900; end
  initial begin #1 p_await = process::self(); p_time.await(); $display("not reached"); end
  initial begin names.get(name); $display("not reached"); end
  initial begin objects.get(taken); $display("not reached"); end
  initial begin p_ready = process::self(); wait (sig == 4); $display("not reached"); end

  initial begin
    #5 p_killed.kill();
    force forced = 3;
    #1 p_finish = process::self();
    sig = 4;
    $display("at the end: box=%0d got=%0d", box.num(), got);
    $finish;
  end

  final begin
    $display("event %s level %s time %s", st(p_event), st(p_level), st(p_time));
    $display("get %s put %s keys %s", st(p_get), st(p_put), st(p_keys));
    $display("join %s done %s killed %s await %s", st(p_join), st(p_done), st(p_killed), st(p_await));
    $display("ready %s finish %s self %s", st(p_ready), st(p_finish), st(process::self()));
    $display("put %0d", box.try_put(5));
    $display("get: box=%0d got=%0d %s", box.num(), got, st(p_get));
    void'(full.try_get(v));
    $display("put: took %0d full=%0d %s", v, full.num(), st(p_put));
    void'(full.try_get(v));
    $display("put: took %0d full=%0d", v, full.num());
    keys.put(2);
    $display("keys: one key %0d %s", keys.try_get(1), st(p_keys));
    void'(names.try_put("text"));
    $display("string: %s left=%0d", name, names.num());
    item.v = 9;
    void'(objects.try_put(item));
    $display("object: %0d left=%0d", taken == null ? -1 : taken.v, objects.num());
    -> go;
    -> gated;
    sig = 5;
    $display("event: triggered=%0d %s conditions=%0d", go.triggered, st(p_event), conditions);
    forced = 7;
    $display("forced: %0d", forced);
    release forced;
    forced = 8;
    $display("released: %0d", forced);
    p_time.kill();
    p_finish.kill();
    $display("kill: %s %s", st(p_time), st(p_finish));
  end

  final begin
    $display("second: %s %s got=%0d", st(p_event), st(p_get), got);
    void'(box.try_put(6));
    $display("second: box=%0d", box.num());
  end
endmodule
