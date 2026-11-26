// A method call or a wait through a null handle of a built-in class ends the
// run with a report at the statement: file, line and column.
`timescale 1ns/1ns
module tb;
  mailbox #(int) m;
  semaphore s;
  event e = null;
  process p;
  int kind, r;
  bit b;

  task t();
    m.get(r);
  endtask
  function int f();
    return m.num();
  endfunction

  initial begin
    if (!$value$plusargs("case=%d", kind)) kind = 0;
    $display("start");
    case (kind)
      0: m.put(1);
      1: t();
      2: m.peek(r);
      3: r = m.num();
      4: r = f();
      5: r = m.try_put(1);
      6: r = m.try_get(r);
      7: r = m.try_peek(r);
      8: s.put(1);
      9: s.get(1);
      10: r = s.try_get(1);
      11: p.kill();
      12: p.suspend();
      13: p.resume();
      14: p.await();
      15: b = p.status() == process::KILLED;
      16: @e;
      17: wait (e.triggered);
      18: wait_order (e);
      19: @(e or posedge b);
      20: @(e or kind);
      21: fork
        @e;
      join
    endcase
    $display("after");
  end
endmodule
