// SIM-015: a task's process variable and its by-value process formal are
// shared with the task's fork branches (SV 9.3.2, 9.7, 13.5.1).
module tb;
  task automatic watch(input process target, output string seen);
    string s;
    fork
      begin #2; s = target.status().name(); end
    join
    seen = s;
  endtask

  task automatic run();
    process h;
    fork
      begin h = process::self(); #10; $display("FAIL killed child resumed"); end
      begin #1; $display("sibling sees %s", h.status().name()); h.kill(); end
    join
    $display("run %s at %0d", h.status().name(), $time);
  endtask

  initial begin
    process w;
    string seen;
    fork
      begin w = process::self(); #5; end
    join_none
    #1;
    watch(w, seen);
    $display("watch %s at %0d", seen, $time);
    run();
    $finish;
  end
endmodule
