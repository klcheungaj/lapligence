// SIM-015: status changes are not change events, so a wait whose condition
// reads status() could never wake; it is rejected explicitly (use await(),
// SV 9.7).
module tb;
  process p;
  initial begin
    fork
      begin p = process::self(); #5; end
    join_none
    #1;
    wait (p.status() == process::FINISHED);
    $display("finished at %0d", $time);
    $finish;
  end
endmodule
