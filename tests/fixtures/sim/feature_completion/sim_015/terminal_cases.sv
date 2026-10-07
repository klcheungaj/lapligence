// SIM-015: self-kill (with a descendant), repeated kill, control of finished
// processes, repeated suspend/resume and await on terminated targets
// (SV 9.7).
module tb;
  process self_k, grand, fin, twice;
  int marks;

  initial begin
    fork
      begin
        self_k = process::self();
        fork begin grand = process::self(); #5; marks += 100; end join_none
        #1;
        process::self().kill();
        marks += 1;
      end
      begin fin = process::self(); end
      begin twice = process::self(); #20; marks += 10; end
    join_none
    #2;
    $display("self-kill %s %s marks=%0d", self_k.status().name(), grand.status().name(), marks);
    fin.kill();
    fin.suspend();
    fin.resume();
    $display("finished %s", fin.status().name());
    fin.await();
    self_k.await();
    $display("await terminal ok %0d", $time);
    twice.suspend();
    twice.suspend();
    $display("suspend twice %s", twice.status().name());
    twice.resume();
    $display("resumed %s", twice.status().name());
    twice.resume();
    twice.kill();
    twice.kill();
    $display("kill twice %s", twice.status().name());
    #30;
    $display("marks=%0d", marks);
    $finish;
  end
endmodule
