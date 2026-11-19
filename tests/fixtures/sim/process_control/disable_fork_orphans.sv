// llg-test-fixture: tests/fixtures/sim/process_control/disable_fork_orphans.sv
// IEEE 1800-2009 9.6.3 (L12548): "The disable fork statement terminates all
// active descendants (subprocesses) of the calling process." L12553-12555:
// "if any of the child processes have descendants of their own, the disable
// fork statement shall terminate them as well." The leaves sit below finished
// processes (a finished child's finished child, a live child's finished
// child, a finished join_any branch); the other initial's tree is untouched.
// The finished processes' status is decision KO-D1 (lrm_decisions).
`timescale 1ns/1ns
module tb;
  process leaf_a, leaf_b, leaf_c, leaf_o;
  task automatic spawn_leaf(int id);
    fork
      begin
        case (id)
          0: leaf_a = process::self();
          1: leaf_b = process::self();
          2: leaf_c = process::self();
          default: leaf_o = process::self();
        endcase
        #50 $display("%0t leaf %0d fired", $time, id);
      end
    join_none
  endtask
  // An unrelated tree that disable fork in the other initial must not reach.
  initial begin
    fork
      begin
        fork begin spawn_leaf(9); #1; end join_none
        #2;
      end
    join_none
  end
  initial begin
    // Direct child that finishes, whose child also finishes: I -> P -> M -> leaf.
    fork
      begin
        fork begin spawn_leaf(0); #2; end join_none
        #1;
      end
    join_none
    // Direct child still alive, with a finished child holding a leaf.
    fork
      begin
        fork begin spawn_leaf(1); #1; end join_none
        #100 $display("%0t live child fired", $time);
      end
    join_none
    // join_any whose slow branch outlives the finished child.
    fork
      begin
        fork
          spawn_leaf(2);
          #1;
        join_any
      end
    join_none
    #20;
    disable fork;
    $display("%0t disable fork", $time);
    #1 $display("%0t leaf_a %s leaf_b %s leaf_c %s leaf_o %s", $time,
                leaf_a.status().name(),
                leaf_b.status().name(), leaf_c.status().name(), leaf_o.status().name());
    #100 $display("%0t end", $time);
    $finish(0);
  end
endmodule
