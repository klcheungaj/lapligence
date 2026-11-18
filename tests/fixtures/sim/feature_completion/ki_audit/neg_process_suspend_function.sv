// `suspend()` inside a function body: a located limit (docs/known_issues.md,
// rejected process-handle forms).
module tb;
  process p;
  function automatic void stop(process q);
    q.suspend();
  endfunction
  initial begin
    fork
      begin p = process::self(); #5 $display("ran"); end
    join_none
    #1 stop(p);
    #10 $finish;
  end
endmodule
