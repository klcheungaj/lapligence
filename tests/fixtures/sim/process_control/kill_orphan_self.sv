// llg-test-fixture: tests/fixtures/sim/process_control/kill_orphan_self.sv
// IEEE 1800-2009 9.7 (L12631-12632), as in kill_orphan_depth.sv: a root kills
// itself and its orphaned leaf; an orphaned leaf kills its own root, which
// ends the leaf itself and its sibling.
`timescale 1ns/1ns
module tb;
  process a, b;
  task automatic spawn_leaf(int id, bit kill_root);
    fork
      begin
        if (kill_root) begin
          #10 $display("%0t leaf %0d kills its root", $time, id);
          b.kill();
          $display("%0t leaf %0d after kill", $time, id);
        end else begin
          #50 $display("%0t leaf %0d fired", $time, id);
        end
      end
    join_none
  endtask
  initial begin
    // Root kills itself after its child finished.
    fork
      begin
        a = process::self();
        fork begin spawn_leaf(1, 0); #1; end join_none
        #20 $display("%0t root a kills itself", $time);
        a.kill();
        $display("%0t root a after kill", $time);
      end
    join_none
    // An orphaned leaf kills its own root, so itself and a sibling leaf.
    fork
      begin
        b = process::self();
        fork begin spawn_leaf(2, 1); spawn_leaf(3, 0); #1; end join_none
        #100 $display("%0t root b fired", $time);
      end
    join_none
    #30 $display("%0t a %s b %s", $time, a.status().name(), b.status().name());
    #100 $display("%0t end", $time);
    $finish(0);
  end
endmodule
