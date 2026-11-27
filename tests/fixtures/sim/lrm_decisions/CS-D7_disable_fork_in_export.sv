// Decision CS-D7: an exported task runs in the process that called the
// import, so `disable fork` in it ends every child of that process: the ones
// it started before the import as well as those the exported task started.
// `process::self()` in the exported task is the caller and `wait fork` waits
// for the caller's children, so `disable fork` ends the same children.
//
// IEEE 1800-2009 9.6.3 (SystemVerilog-1800-2009.txt L12548):
//   "The disable fork statement terminates all active descendants (subprocesses) of the calling process."
// 9.6.1 (L12335-12336):
//   "The wait fork statement blocks process execution flow until all immediate child subprocesses (processes
//   created by the current process, excluding their descendants) have completed their execution."
// 35.8 (L56248-56249):
//   "The return value of an exported task is an int value that indicates if a disable is active or
//   not on the current execution thread."
// 35.5.1.5 (L55696-55698):
//   "A call to an imported task can result in the suspension of the currently executing thread. This occurs when an
//   imported task calls an exported task, and the exported task executes a delay control, event control, or wait
//   statement."
// An industry-standard simulator ends only the children that the exported
// task started: it also prints "child A survived", "child C survived" and
// "child D survived". llg follows 9.6.3.
//
// Build CS-D7_disable_fork_in_export.c into a shared library and load it with
// the simulator's DPI library option.
`timescale 1ns/1ns
module tb;
  import "DPI-C" context task cs7_call(input int mode);
  export "DPI-C" task cs7_export;
  process caller;

  task automatic cs7_export(input int mode);
    if (mode == 0) begin
      $display("[%0t] export runs in the caller: %0d", $time, process::self() == caller);
      wait fork;
      $display("[%0t] export waited for the caller's child", $time);
    end else begin
      if (mode == 2) begin
        fork begin #2 $display("[%0t] child of the export survived", $time); end join_none
      end
      disable fork;
      $display("[%0t] export after disable fork", $time);
    end
  endtask

  task automatic direct;
    disable fork;
  endtask

  initial begin
    caller = process::self();
    fork begin #2 $display("[%0t] child W ends", $time); end join_none
    #1 cs7_call(0);
    $display("[%0t] after wait fork in the export", $time);
    fork begin #2 $display("[%0t] child A survived", $time); end join_none
    #1 cs7_call(1);
    $display("[%0t] after disable fork in the export", $time);
    #5;
    fork begin #2 $display("[%0t] child B survived", $time); end join_none
    #1 direct();
    $display("[%0t] after disable fork in a task", $time);
    #5;
    fork begin #2 $display("[%0t] child C survived", $time); end join_none
    #1 cs7_call(2);
    $display("[%0t] after the export's own child", $time);
    #5;
    fork
      begin
        fork begin #2 $display("[%0t] child D survived", $time); end join_none
        #1 cs7_call(1);
        $display("[%0t] branch after the export", $time);
        #3;
      end
      begin #2 $display("[%0t] sibling survived", $time); end
    join
    $finish;
  end
endmodule
