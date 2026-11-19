// KO-D1: kill() and disable fork reach the subprocesses of finished
// processes; a process that finished keeps FINISHED.
//
// IEEE 1800-2009 9.7 (L12631-12632): "The kill() function terminates the given
// process and all its subprocesses, that is, processes spawned using fork
// statements by the process being killed."
// 9.6.3 (L12548): "The disable fork statement terminates all active
// descendants (subprocesses) of the calling process." (L12553-12555): "The
// disable fork statement terminates all descendants of the calling process as
// well as the descendants of the process's descendants. In other words, if any
// of the child processes have descendants of their own, the disable fork
// statement shall terminate them as well."
// 9.7 (L12615, L12619): "FINISHED means the process terminated normally."
// "KILLED means the process was forcibly killed (via kill or disable)."
//
// Decision: a process stays a subprocess of each ancestor after its parent
// has finished, so kill() of an ancestor and disable fork in an ancestor
// terminate it. kill() of a process that has already finished terminates the
// subprocesses it left running. A process that finished normally keeps
// FINISHED when an ancestor is killed or runs disable fork, also while it
// still had live subprocesses.
`timescale 1ns / 1ns
module tb;
  process r, z, w, p, leaf, x, y;

  initial begin
    // kill() of a live root whose child z finished and left w running.
    fork
      begin
        r = process::self();
        fork
          begin
            z = process::self();
            fork begin w = process::self(); #100 $display("unexpected w"); end join_none
            #1;
          end
        join_none
        #200;
      end
    join_none
    #10 r.kill();
    #1 $display("kill: r %s z %s w %s", r.status().name(), z.status().name(),
                w.status().name());
    // kill() of a process that has finished; its child still runs.
    fork
      begin
        p = process::self();
        fork begin leaf = process::self(); #100 $display("unexpected leaf"); end join_none
        #1;
      end
    join_none
    #10 p.kill();
    #1 $display("kill finished: p %s leaf %s", p.status().name(), leaf.status().name());
    // disable fork while the child x has finished and left y running.
    fork
      begin
        x = process::self();
        fork begin y = process::self(); #100 $display("unexpected y"); end join_none
        #1;
      end
    join_none
    #10 disable fork;
    #1 $display("disable fork: x %s y %s", x.status().name(), y.status().name());
    #200 $display("done");
    $finish(0);
  end
endmodule
