// `status()` in a wait condition: a status change publishes no event, so the
// form is a located limit (docs/known_issues.md, rejected process-handle forms).
module tb;
  process p;
  initial begin
    fork
      begin p = process::self(); #2; end
    join_none
    #1;
    wait (p.status() == process::FINISHED);
    $display("done");
    $finish;
  end
endmodule
