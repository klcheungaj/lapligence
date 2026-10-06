// SIM-015: a process woken by an event but killed before it runs never
// resumes; two processes woken together may kill each other in either order
// (SV 9.7, 4.7). The test accepts each permitted outcome.
module tb;
  process victim, a, b;
  event go, pair;

  initial begin
    fork
      begin victim = process::self(); @go; $display("victim ran"); end
      begin a = process::self(); @pair; $display("a woke"); b.kill(); end
      begin b = process::self(); @pair; $display("b woke"); a.kill(); end
    join_none
    #1;
    ->go;
    victim.kill();
    $display("victim %s", victim.status().name());
    #1;
    ->pair;
    #1;
    $display("pair %s %s", a.status().name(), b.status().name());
    $finish;
  end
endmodule
