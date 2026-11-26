// S09-D10: in which order the processes of consecutive forks start.
// 9.3.2 (SystemVerilog-1800-2009.txt L11430-11432): "The parent process
// continues to execute concurrently with all the processes spawned by the
// fork. The spawned processes do not start executing until the parent thread
// executes a blocking statement."
// Decision: fork ... join and fork ... join_any are blocking statements: the
// children of pending join_none forks start, in source order, before those of
// the fork that the same process executes next.
`timescale 1ns/1ns
module tb;
  task automatic t(string s);
    $display("%s", s);
  endtask
  task automatic nested;
    fork t("nested.none1"); join_none
    fork t("nested.join1"); t("nested.join2"); join
    t("nested.after");
    fork t("nested.none2"); join_none
    fork t("nested.any"); join_any
    t("nested.end");
  endtask
  initial begin
    fork t("a.none1"); t("a.none2"); join_none
    fork t("a.join1"); t("a.join2"); join
    t("a.after");
    #1 fork t("b.none1"); join_none
    fork t("b.none2"); join_none
    fork t("b.join"); join
    t("b.after");
    #1 fork t("c.none"); join_none
    fork t("c.any"); #2; join_any
    t("c.after");
    #1 fork
      begin
        t("d.none");
        #1 t("d.none.end");
      end
    join_none
    fork t("d.join1"); begin t("d.join2"); #2 t("d.join2.end"); end join
    t("d.after");
    #1 nested();
    #1 fork t("e.none1"); join_none
    fork
      begin
        fork t("e.none2"); join_none
        fork t("e.inner"); join
        t("e.outer");
      end
    join
    fork t("e.none3"); join_none
    t("e.last");
    #1 $finish(0);
  end
endmodule
