// `try_get` of a mailbox into a process variable: a located limit
// (docs/known_issues.md, rejected process-handle forms).
module tb;
  mailbox #(process) mb = new;
  process q;
  initial begin
    mb.put(process::self());
    if (mb.try_get(q) > 0) $display("got");
    $finish;
  end
endmodule
