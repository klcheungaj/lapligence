// status() in an event expression: a located limit (docs/known_issues.md,
// rejected process-handle forms).
module tb;
  process p;
  initial begin
    fork begin p = process::self(); #1; end join_none
    #0;
    @(p.status());
    $display("%0t", $time);
    $finish(0);
  end
endmodule
