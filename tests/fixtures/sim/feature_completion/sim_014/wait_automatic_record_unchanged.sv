// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_014/wait_automatic_record_unchanged.sv
// The neighbour of reject_wait_automatic_record that stays legal: an
// automatic record with a string member that fork branches read, with every
// store before the fork or after its join. Nothing can store into it while a
// branch waits, so the wait needs no change of it (SV 9.4.3).
typedef struct { int limit; string name; } cfg_t;
module tb;
  timeunit 1ns; timeprecision 1ns;
  int cnt;
  task automatic run(int n);
    cfg_t cfg;
    cfg.limit = n;
    cfg.name = "c";
    fork
      begin
        wait (cnt == cfg.limit);
        $display("%0t %s reached %0d", $time, cfg.name, cfg.limit);
      end
      begin repeat (n) #1 cnt++; end
    join
    cfg.limit = 0;
  endtask
  initial begin
    run(3);
    $finish(0);
  end
endmodule
