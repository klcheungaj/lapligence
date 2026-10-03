// SV2009 9.6.1-9.6.3: disabling a process suspended on a process-evaluated
// event detaches its wait; later dependency changes neither resume it nor
// evaluate its helper again.
module tb;
  int a = 0, evals = 0, after = 0;
  function int f(input int value);
    evals++;
    return value;
  endfunction
  task automatic watch(input int id);
    @(f(a)) $display("late %0d", id);
  endtask
  initial begin
    fork : watchers
      begin @(f(a)) $display("late block"); end
      watch(1);
    join_none
    #1 disable watchers;
    after = evals;
    #1 a = 5;
    #1 $display("cancelled evaluated=%0d unchanged=%0d", after > 0, evals == after);
    fork
      begin @(f(a)) $display("live %0d %0t", a, $time); end
      #1 a = 6;
    join_any
    disable fork;
    $finish(0);
  end
endmodule
