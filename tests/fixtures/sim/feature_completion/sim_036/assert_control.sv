// SIM-036 A02: $assertoff, $assertkill and $asserton with deferred
// assertions (IEEE 1800-2009 20.11, 16.4.1). Reports queued before
// $assertoff still mature; $assertkill flushes reports that have not
// matured; a matured report still executes when an earlier action of the
// same Reactive region calls $assertkill (decision S36-D5).
module tb;
  task killer();
    $display("%0d killer", $time);
    $assertkill;
  endtask

  initial begin
    a1: assert #0 (1'b0) else $display("%0d a1 queued before off", $time);
    $assertoff;
    a2: assert #0 (1'b0) else $display("%0d BAD a2 checked while off", $time);
    #1 $asserton;
    a3: assert #0 (1'b0) else $display("%0d BAD a3 killed", $time);
    $assertkill;
    #1 $asserton;
    a4: assert #0 (1'b0) else killer();
    a5: assert #0 (1'b0) else $display("%0d a5 matured before the kill", $time);
    #1 a6: assert #0 (1'b0) else $display("%0d BAD a6 checked after kill", $time);
    $asserton;
    a7: assert #0 (1'b0) else $display("%0d a7 after asserton", $time);
    #1 $finish(0);
  end
endmodule
