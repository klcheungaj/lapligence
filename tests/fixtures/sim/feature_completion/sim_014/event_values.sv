// SIM-014: event values and triggered state (SV 15.5.3, 15.5.4, 15.5.5).
// `.triggered` holds for the rest of the time step; `wait (e.triggered)`
// unblocks even when the trigger ran first; a null event's `.triggered` is
// false and triggering it has no effect; events compare by synchronization
// object and test false only when null; assignment merges queues for later
// waits only; wait_order succeeds when the events trigger in order (earlier
// ones may retrigger) and takes its else branch otherwise; the first event
// may be satisfied by its triggered state.
`timescale 1ns / 1ns
module tb;
  event a, b, c, n1, al, e1, e2;
  bit ok;
  int got;

  initial begin
    ->a;
    $display("t0 a.triggered=%0d", a.triggered);
    #0 $display("t0 #0 a.triggered=%0d", a.triggered);
    #1 $display("t1 a.triggered=%0d", a.triggered);
    fork
      ->b;
      wait (b.triggered);
    join
    $display("%0t wait triggered done", $time);
    n1 = null;
    $display("null triggered=%0d", n1.triggered);
    ->n1;
    al = a;
    $display("compare %0d %0d %0d %0d %0d %0d", al == a, al != a, al === a, al !== a,
             al == null, a == b);
    if (n1) $display("n1 nonnull");
    else $display("n1 null");
    if (!al) $display("al null");
    else $display("al nonnull");
    $display("ternary %0d %0d", n1 ? 1 : 0, al ? 1 : 0);
    fork
      begin
        @al got++;
      end
      #1 ->a;
    join
    $display("%0t merged got=%0d", $time, got);
    fork
      begin
        @e1 $display("%0t old e1 waiter woke", $time);
      end
      begin
        #1 e1 = e2;
        ->e1;
        $display("%0t e1 now e2: %0d", $time, e1 == e2);
      end
    join_none
    #2;
    fork
      begin
        wait_order (a, b, c) ok = 1;
        else ok = 0;
        $display("%0t order ok=%0d", $time, ok);
      end
      begin
        #1 ->a;
        #1 ->b;
        #1 ->a;
        #1 ->c;
      end
    join
    fork
      begin
        wait_order (a, b, c) ok = 1;
        else ok = 0;
        $display("%0t order ok=%0d", $time, ok);
      end
      begin
        #1 ->a;
        #1 ->c;
        #1 ->b;
      end
    join
    fork
      begin
        wait_order (a, b) else $display("%0t order else only", $time);
      end
      #1 ->b;
    join
    ->a;
    fork
      begin
        wait_order (a, b) $display("%0t first event already triggered", $time);
        else $display("%0t failed", $time);
      end
      #1 ->b;
    join
    $finish;
  end
endmodule
