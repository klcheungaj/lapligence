// status() in a continuous assignment: a located limit (docs/known_issues.md,
// rejected process-handle forms).
module tb;
  process p;
  wire done = (p != null) && (p.status() == process::FINISHED);
  initial begin
    fork begin p = process::self(); #1; end join_none
    #2 $display("%0d", done);
    $finish(0);
  end
endmodule
