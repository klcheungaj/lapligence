// Static-review counterexample; NOT EXECUTED.
module tb;
  semaphore sem;
  process first;
  bit acquired;
  initial begin
    sem = new(1);
    fork
      begin first = process::self(); sem.get(2); end
      begin #1; sem.get(1); acquired = 1; end
      begin #2; first.kill(); end
    join
  end
  initial begin
    #4;
    if (!acquired) $fatal(1, "satisfiable waiter remained blocked after head cancellation");
    $finish(0);
  end
endmodule
