// SIM-015: a terminated process no longer owns a random stream (SV 18.14);
// seeding it through a retained handle is reported and has no effect.
module tb;
  process done_p;
  initial begin
    fork
      begin done_p = process::self(); end
    join_none
    #1;
    done_p.srandom(5);
    $display("status %s", done_p.status().name());
    $finish;
  end
endmodule
