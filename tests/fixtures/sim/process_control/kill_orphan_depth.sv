// llg-test-fixture: tests/fixtures/sim/process_control/kill_orphan_depth.sv
// IEEE 1800-2009 9.7 (L12631-12632): kill() "terminates the given process and
// all its subprocesses, that is, processes spawned using fork statements by the
// process being killed". Root d's leaf is d generations down and every
// process between them has finished: killing roots 2 to 4 ends their leaves;
// root 5 is not killed and its leaf fires.
`timescale 1ns/1ns
module tb;
  process root[2:5];
  process leaf[2:5];
  // Each intermediate process forks the next generation and then finishes,
  // so the leaf's parent chain up to the root has already ended.
  task automatic spawn(int gen, int id);
    fork
      begin
        if (gen == 1) begin
          leaf[id] = process::self();
          #50 $display("%0t leaf %0d fired", $time, id);
        end else begin
          spawn(gen - 1, id);
          #1;
        end
      end
    join_none
  endtask
  initial begin
    for (int d = 2; d <= 5; d++) begin
      automatic int dd = d;
      fork
        begin
          root[dd] = process::self();
          spawn(dd, dd);
          #100;
        end
      join_none
    end
    #20;
    root[2].kill();
    root[3].kill();
    root[4].kill();
    $display("%0t killed roots 2..4", $time);
    #1;
    for (int d = 2; d <= 5; d++)
      $display("%0t root %0d %s leaf %s", $time, d, root[d].status().name(), leaf[d].status().name());
    #100 $display("%0t end", $time);
    $finish(0);
  end
endmodule
