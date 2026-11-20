// Process-handle forms that docs/known_issues.md listed as rejected and llg
// serves: `ref` process formals bound to elements of a fixed array and a
// queue, `await()` in a fork-join_none branch of a function, and status()
// read in an always_comb body (whose sensitivity holds variables only).
module tb;
  timeunit 1ns;
  timeprecision 1ns;
  process ps[2];
  process pq[$];
  process p;
  bit done;
  task automatic take(ref process h);
    h = process::self();
  endtask
  function automatic void watch();
    fork begin p.await(); $display("finished at %0t", $time); end join_none
  endfunction
  always_comb done = (p != null) && (p.status() == process::FINISHED);
  initial begin
    pq.push_back(null);
    take(ps[1]);
    take(pq[0]);
    $display("%0d %0d %0d", ps[0] != null, ps[1] != null, pq[0] != null);
  end
  initial begin
    fork begin p = process::self(); #3; end join_none
    #1 watch();
    #4 $display("done %0d", done);
    $finish(0);
  end
endmodule
