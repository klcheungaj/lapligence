// Decision S25-D10: every $strobe call prints a line of its own.
//
// IEEE 1800-2009 21.2.2 (SystemVerilog-1800-2009.txt L36482-36484):
//   "The system task $strobe provides the ability to display simulation data
//   at a selected time. That time is the end of the current simulation time,
//   when all the simulation events have occurred for that simulation time,
//   just before simulation time is advanced."
//
// The clause sets when each call displays and nothing merges calls, so a call
// site reached twice in a slot, through a task or a loop, prints twice, with
// the values of the end of the slot, the last issued first (S25-D6). Some
// simulators print one line per call site per slot; llg follows the clause
// text because it gives no reason to drop a call.
module tb;
  int v;

  task report(int k);
    $strobe("task v=%0d", v);
  endtask

  initial begin
    report(1);
    report(2);
    for (int i = 0; i < 2; i++) $strobe("loop v=%0d", v);
    v = 7;
    #1 $finish;
  end
endmodule
