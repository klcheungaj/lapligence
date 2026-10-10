// SIM-036 A02: disable and deferred assertions (IEEE 1800-2009 16.4.4).
// Disabling one deferred assertion cancels its pending reports; disabling
// the outermost scope of a procedure flushes that procedure's queue, also
// while the procedure waits at its leading event control; disabling an
// inner scope flushes nothing.
module tb;
  logic bad_val = 1'b1, bad_val_ok = 1'b0;
  logic [7:0] c = 8'd0;
  logic clear = 1'b0;
  logic t = 1'b0, u = 1'b1;

  // The first 16.4.4 example.
  always @(bad_val or bad_val_ok) begin : b1
    a1: assert #0 (bad_val) else $display("%0d a1 fail ok=%0d", $time, bad_val_ok);
    if (bad_val_ok) begin
      disable a1;
    end
  end

  // The second 16.4.4 example, made race-free: b2 runs in the Active region
  // and b3 disables it from the Inactive region of the same time step.
  always @(c) begin : b2
    a3: assert #0 (c == 8'd0) else $display("%0d a3 fail c=%0d", $time, c);
  end
  always @(clear) begin : b3
    disable b2;
  end

  // Disabling a non-outermost scope does not flush.
  always @(t or u) begin : outer
    begin : inner
      a4: assert #0 (u) else $display("%0d a4 fail kept", $time);
      if (t) disable inner;
    end
  end

  initial begin
    #1 bad_val = 1'b0;               // a1 fails: reported
    #1 bad_val = 1'b1;               // a1 passes
    #1 begin
      bad_val = 1'b0;                // a1 fails, then is disabled: cancelled
      bad_val_ok = 1'b1;
    end
    #1 c = 8'd1;                     // a3 fails: reported
    #1 begin
      c = 8'd2;                      // a3 fails ...
      #0 clear = 1'b1;               // ... and b2 is disabled: flushed
    end
    #1 begin
      u = 1'b0;                      // a4 fails; inner is disabled: kept
      t = 1'b1;
    end
    #1 $finish(0);
  end
endmodule
