// SIM-009-A03 boundary: legal (SV 9.4.2, 13.5.2), but an element of an
// unpacked array of strings has no change marker a task waiting on it
// through a string `ref` formal can follow; the call is rejected instead of
// never waking.
module tb;
  string names[2];

  task automatic ws(ref string s);
    wait (s == "go");
    $display("ws at %0d", $time);
  endtask

  initial begin
    fork
      ws(names[1]);
    join_none
    #1 names[1] = "go";
    #1 $finish;
  end
endmodule
