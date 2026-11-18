// A deferred action passing a dynamic array by value: the action frame keeps
// no owned copy of a container, a located limit (docs/known_issues.md,
// deferred assertion actions with container, event or virtual-interface
// arguments).
module tb;
  int d[];
  task automatic show(input int x[]);
    $display("[%0t] size=%0d", $time, x.size());
  endtask
  initial begin
    d = new[2];
    assert #0 (0) else show(d);
    #1 $finish;
  end
endmodule
