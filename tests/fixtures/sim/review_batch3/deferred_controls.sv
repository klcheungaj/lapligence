// Static-review regression; not executed during patch preparation.
`timescale 1ns/1ps
module tb;
  int evaluations = 0;
  int reports = 0;
  function bit condition();
    evaluations++;
    return 0;
  endfunction
  function void report_failure();
    reports++;
  endfunction
  initial begin
    // Repeat one module-process assertion site; task-body deferred assertions
    // remain outside the existing admitted subset.
    for (int step = 0; step < 5; step++) begin
      if (step == 2 || step == 4) $asserton;
      deferred_check: assert #0 (condition()) else report_failure();
      if (step == 0) $assertoff; // Queued report must still mature.
      if (step == 2) $assertkill; // Queued report must be flushed.
      #1;
      if (step < 2 && (reports != 1 || evaluations != 1))
        $fatal(1, "off flushed a report or evaluated a disabled assertion");
      if (step >= 2 && step < 4 && (reports != 1 || evaluations != 2))
        $fatal(1, "kill failed to flush reports or disable new checks");
    end
    if (reports != 2 || evaluations != 3) $fatal(1, "deferred restart");
    $display("deferred controls ok");
    $finish(0);
  end
endmodule
