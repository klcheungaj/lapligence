// FIN-001 A3 limit: a deferred action passing a queue by value (IEEE
// 1800-2009 16.4: "Actual argument expressions that are passed by value use
// the values of the underlying variables at the instant the deferred
// assertion expression was evaluated") needs a copy of the queue in the
// action frame, which llg does not make; the call is rejected at the action.
module tb;
  int q[$];
  task automatic show(input int x[$]);
    $display("[%0t] size=%0d", $time, x.size());
  endtask
  initial begin
    q.push_back(5);
    assert #0 (0) else show(q);
    #1 $finish;
  end
endmodule
