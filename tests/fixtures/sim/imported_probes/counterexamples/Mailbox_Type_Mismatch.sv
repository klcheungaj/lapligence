// Static-review counterexample; NOT EXECUTED.
module tb;
  mailbox m;
  string s;
  int status;
  initial begin
    m = new;
    m.put(123);
    status = m.try_get(s);
    if (status >= 0) $fatal(1, "try_get type mismatch must return a negative integer");
    if (m.num() != 1) $fatal(1, "failed retrieval removed a message");
    $finish(0);
  end
endmodule
