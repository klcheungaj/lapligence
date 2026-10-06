// SIM-010: mutually recursive timed tasks fork detached children that
// outlive every activation's return and read that activation's shared
// formal; disabling one sibling branch by name leaves its siblings running
// (SV 9.3.2, 9.6.2, 13.3).
module tb;
  task automatic ping(int n);
    fork
      begin #5 $display("ping child %0d %0d", n, $time); end
    join_none
    if (n > 0) begin #1 pong(n - 1); end
  endtask

  task automatic pong(int n);
    fork
      begin : keep #3 $display("pong keep %0d %0d", n, $time); end
      begin : drop #2 $display("FAIL pong drop %0d", n); end
      begin #1 disable drop; end
    join_none
    if (n > 0) begin #1 ping(n - 1); end
  endtask

  initial begin
    ping(3);
    $display("returned %0d", $time);
    #10 $finish;
  end
endmodule
