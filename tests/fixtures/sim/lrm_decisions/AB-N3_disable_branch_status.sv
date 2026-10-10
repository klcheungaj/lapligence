// AB-N3: status() of a fork branch whose whole body is a named block that
// another process disables.
//
// IEEE 1800-2009 9.7 (L12615-12619): "FINISHED means the process terminated
// normally. ... KILLED means the process was forcibly killed (via kill or
// disable)."
// 9.6.2 (L12387-12388): "The disable statement shall terminate the activity
// of a task or a named block. Execution shall resume at the statement
// following the block or following the task-enabling statement."
//
// Decision: when the disabled block is the entire body of a fork branch,
// nothing follows it, so disabling it terminates the branch's process
// forcibly: status() is KILLED, as after kill() or disable fork. A disabled
// block followed by more statements only skips the block; the process goes on
// and later finishes normally (FINISHED).
`timescale 1ns / 1ns
module tb;
  process whole, part;
  int after;

  initial begin
    after = 0;
    fork
      begin : whole_blk
        whole = process::self();
        #10 $display("unexpected whole");
      end
      begin
        part = process::self();
        begin : part_blk
          #10 $display("unexpected part");
        end
        after = 1;
      end
    join_none
    #1 disable whole_blk;
    disable part_blk;
    #1 $display("whole branch: %s", whole.status().name());
    $display("partial block: %s after=%0d", part.status().name(), after);
    $finish(0);
  end
endmodule
