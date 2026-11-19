// llg-test-fixture: tests/fixtures/sim/process_control/kill_orphan_status.sv
// IEEE 1800-2009 9.7 (L12615-12619, L12631-12632), as in kill_orphan_await.sv:
// after the root's kill its live descendants are KILLED and its finished
// ones, with or without live children, stay FINISHED.
`timescale 1ns/1ns
module tb;
  process r, a, b, c, x, y, z, w;
  initial begin
    fork
      begin
        r = process::self();
        fork begin a = process::self(); #1; end join_none
        fork
          begin b = process::self(); #1; end
          begin c = process::self(); #100; end
        join_none
        fork begin x = process::self(); fork begin y = process::self(); #1; end join_none #2; end join_none
        fork begin z = process::self(); fork begin w = process::self(); #100; end join_none #2; end join_none
        #200;
      end
    join_none
    #10 $display("%0t before a %s b %s c %s x %s y %s z %s w %s", $time, a.status().name(), b.status().name(),
                 c.status().name(), x.status().name(), y.status().name(), z.status().name(), w.status().name());
    r.kill();
    #1 $display("%0t after r %s a %s b %s c %s x %s y %s z %s w %s", $time, r.status().name(), a.status().name(),
                b.status().name(), c.status().name(), x.status().name(), y.status().name(), z.status().name(),
                w.status().name());
    $finish(0);
  end
endmodule
