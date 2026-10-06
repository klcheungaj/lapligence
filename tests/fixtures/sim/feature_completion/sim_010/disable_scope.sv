// SIM-010: `disable fork` ends only the calling process's descendants,
// including branches forked inside a task it called (SV 9.6.3); disabling a
// task by name ends every activation of it (SV 9.6.2).
module tb;
  task automatic worker(int id);
    fork
      begin #5 $display("w%0d branch %0d", id, $time); end
    join_none
    #10 $display("w%0d done %0d", id, $time);
  endtask

  task automatic sleeper(int id);
    #4 $display("s%0d woke %0d", id, $time);
  endtask

  initial begin : a
    fork worker(1); join_none
    #2 disable fork;
    $display("a disabled fork %0d", $time);
  end

  initial begin : b
    fork worker(2); join_none
  end

  initial begin
    #20;
    fork sleeper(3); sleeper(4); join_none
    #1 disable sleeper;
    $display("sleepers disabled %0d", $time);
    #10 $finish;
  end
endmodule
