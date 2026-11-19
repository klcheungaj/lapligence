// llg-test-fixture: tests/fixtures/sim/process_control/kill_orphan_await.sv
// IEEE 1800-2009 9.7 (L12615-12619): "FINISHED means the process terminated
// normally. ... KILLED means the process was forcibly killed (via kill or
// disable)." L12631-12632 as in kill_orphan_depth.sv. An await() of the
// orphaned leaf returns at the kill; the finished middle process stays
// FINISHED.
`timescale 1ns/1ns
module tb;
  process root, mid, leaf;
  initial begin
    fork
      begin
        root = process::self();
        fork
          begin
            mid = process::self();
            fork begin leaf = process::self(); #50 $display("%0t leaf fired", $time); end join_none
            #5;
          end
        join_none
        #100;
      end
    join_none
    // A watcher awaits the orphaned leaf; another awaits the finished middle process.
    fork
      begin #1 leaf.await(); $display("%0t leaf await returned %s", $time, leaf.status().name()); end
      begin #1 mid.await(); $display("%0t mid await returned %s", $time, mid.status().name()); end
    join_none
    #10 $display("%0t before kill root %s mid %s leaf %s", $time, root.status().name(),
                 mid.status().name(), leaf.status().name());
    #10 root.kill();
    $display("%0t after kill root %s mid %s leaf %s", $time, root.status().name(),
             mid.status().name(), leaf.status().name());
    #100 $display("%0t end", $time);
    $finish(0);
  end
endmodule
