// llg-test-fixture: tests/fixtures/sim/process_control/wait_fork_orphans.sv
// IEEE 1800-2009 9.6.1 (L12335-12336): "The wait fork statement blocks process
// execution flow until all immediate child subprocesses (processes created by
// the current process, excluding their descendants) have completed their
// execution." 9.6.3 (L12553-12555): "if any of the child processes have
// descendants of their own, the disable fork statement shall terminate them
// as well." wait fork returns when the child finishes; disable fork then ends
// the grandchild the child left running.
`timescale 1ns/1ns
module tb;
  task automatic spawn();
    fork
      begin
        fork #50 $display("%0t grandchild fired", $time); join_none
        #5 $display("%0t child done", $time);
      end
    join_none
  endtask
  initial begin
    fork
      begin
        spawn();
        // wait fork waits only for the immediate child, not its orphan.
        wait fork;
        $display("%0t wait fork returned", $time);
        disable fork;
        $display("%0t disable fork", $time);
        #10;
      end
    join_none
    #100 $display("%0t end", $time);
    $finish(0);
  end
endmodule
