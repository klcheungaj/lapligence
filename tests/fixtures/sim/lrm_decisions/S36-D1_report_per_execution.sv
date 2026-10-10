// Decision S36-D1: every execution of a deferred assertion queues its own
// report; only a flush point clears the queue.
//
// IEEE 1800-2009 16.4.1 (SystemVerilog-1800-2009.txt L21243-21250):
//   "the action block subroutine call ... and the current values of its input
//   arguments are placed in a deferred assertion report queue associated with
//   the currently executing process. ... If a deferred assertion flush point
//   (see 16.4.2) is reached in a process, its deferred assertion report queue
//   is cleared."
// 16.4.2 (L21276-21281) lists the flush points; executing the same assertion
// again is not one of them.
//
// A loop that executes one assertion three times without a flush point
// therefore reports three times, each with its own issue-time argument. (llg
// formerly kept only the last result of one assertion per process and time
// step.)
module tb;
  task automatic report(input int i);
    $display("report i=%0d", i);
  endtask
  initial begin
    for (int i = 0; i < 3; i++)
      assert #0 (i == 5) else report(i);
    #1 $finish;
  end
endmodule
