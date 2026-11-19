// llg-test-fixture: tests/fixtures/sim/process_control/kill_orphan_joins.sv
// IEEE 1800-2009 9.7 (L12631-12632), as in kill_orphan_depth.sv: the leaves
// outlive a finished join_any parent and a finished joined branch.
`timescale 1ns/1ns
module tb;
  process root, any_slow, inner_leaf, sib;
  task automatic body();
    // join_any: the slow branch outlives this process.
    fork
      begin any_slow = process::self(); #50 $display("%0t join_any slow branch fired", $time); end
      #3 $display("%0t join_any fast branch done", $time);
    join_any
    // join: the joined branch forks a join_none grandchild and finishes.
    fork
      begin
        fork
          begin inner_leaf = process::self(); #60 $display("%0t inner leaf fired", $time); end
        join_none
        #2 $display("%0t joined branch done", $time);
      end
    join
    $display("%0t body done", $time);
  endtask
  initial begin
    fork
      begin
        root = process::self();
        fork
          body();
          begin sib = process::self(); #70 $display("%0t live sibling fired", $time); end
        join_none
        #100;
      end
    join_none
    #20;
    $display("%0t kill root", $time);
    root.kill();
    #1 $display("%0t root %s any_slow %s inner_leaf %s sib %s", $time, root.status().name(),
                any_slow.status().name(), inner_leaf.status().name(), sib.status().name());
    #100 $display("%0t end", $time);
    $finish(0);
  end
endmodule
