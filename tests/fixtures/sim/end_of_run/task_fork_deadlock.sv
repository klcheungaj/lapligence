// A fork inside a task called from an initial procedure leaves a blocked
// child behind after the task returns.
module tb;
  event ev;
  task automatic spawn_child;
    fork
      @(ev);
    join_none
  endtask
  initial begin
    spawn_child();
    #1 $display("parent done");
  end
  final $display("final");
endmodule
