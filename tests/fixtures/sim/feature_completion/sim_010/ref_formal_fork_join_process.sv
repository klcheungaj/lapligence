// Process-handle `ref` formals in `fork ... join` branches (IEEE 1800-2009
// 9.3.2, 13.5.2): a branch stores the handle, siblings and the caller see it
// at once, and a store through the formal wakes the caller's watchers.
`timescale 1ns/1ns
module tb;
  process h;
  int watched = -1;
  always @(h) watched = int'($time);
  task automatic t(ref process r);
    fork
      begin #1 r = process::self(); end
      begin
        wait (r != null);
        $display("%0t seen %0d", $time, r.status() == process::FINISHED);
      end
      #2 $display("%0t null %0d", $time, r == null);
    join
  endtask
  task automatic nested(ref process r);
    fork
      begin
        fork
          begin #1 r = process::self(); #1; end
        join
      end
      begin
        wait (r != null);
        $display("%0t nested seen %0d", $time, r.status() == process::RUNNING);
      end
    join
  endtask
  task automatic local_actual();
    process lp;
    nested(lp);
    $display("%0t local %0d", $time, lp.status() == process::FINISHED);
  endtask
  initial begin
    t(h);
    $display("%0t caller %0d %0d watched %0d", $time, h == null,
             h.status() == process::FINISHED, watched);
    local_actual();
    $finish(0);
  end
endmodule
