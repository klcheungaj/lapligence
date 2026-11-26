// Legal by SV 13.5.2. A cell of a fixed array of queues passed to a `ref`
// formal is an alias that publishes its changes after the call; a task can
// suspend while another process waits on the array, so only functions take
// one.
module tb;
  int q[2][$];
  task automatic late(ref int t[$]);
    #1 t.push_back(1);
  endtask
  initial begin
    late(q[1]);
    $finish;
  end
endmodule
