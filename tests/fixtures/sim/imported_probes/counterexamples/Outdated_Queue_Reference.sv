// Static-review counterexample; NOT EXECUTED.
module tb;
  int q[$];
  task automatic retain(ref int element);
    q.delete(0);
    element = 12;
    if (element != 12) $fatal(1, "outdated reference lost its private element cell");
    if (q.size() != 0) $fatal(1, "outdated reference modified the queue");
  endtask
  initial begin
    q.push_back(7);
    retain(q[0]);
    $finish(0);
  end
endmodule
