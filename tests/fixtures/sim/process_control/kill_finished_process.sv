// llg-test-fixture: tests/fixtures/sim/process_control/kill_finished_process.sv
// IEEE 1800-2009 9.7 (L12631-12632), as in kill_orphan_depth.sv: kill() of a
// process that has finished still ends the subprocess it left running; the
// process itself stays FINISHED (L12615).
`timescale 1ns/1ns
module tb;
  process p, leaf;
  initial begin
    // The killed process has itself finished; its child still runs.
    fork
      begin
        p = process::self();
        fork begin leaf = process::self(); #50 $display("%0t leaf fired", $time); end join_none
        #5;
      end
    join_none
    #20 $display("%0t before kill p %s leaf %s", $time, p.status().name(), leaf.status().name());
    p.kill();
    #1 $display("%0t after kill p %s leaf %s", $time, p.status().name(), leaf.status().name());
    #100 $display("%0t end", $time);
    $finish(0);
  end
endmodule
