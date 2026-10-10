// SIM-009-A03: legal (SV 9.4.2, 13.5.2), but a task-local string that a fork
// branch can change has no change marker, so a task waiting on it through a
// `ref` formal is rejected explicitly instead of never waking.
module tb;
  task automatic ws(ref string s);
    wait (s == "go");
    $display("ws at %0d", $time);
  endtask

  task automatic owner();
    string s;
    fork ws(s); join_none
    #1 s = "go";
    #1;
  endtask

  initial begin
    owner();
    $finish;
  end
endmodule
