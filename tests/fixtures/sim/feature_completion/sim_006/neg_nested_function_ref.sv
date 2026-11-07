// Legal by SV 13.5.2; a queue nested in an element has only a staged copy, which
// a reference must not alias, so a function formal is rejected as a task's is.
module tb;
  int qq[$][$];
  int qq0[$];
  function automatic void add(ref int t[$]);
    t.push_back(1);
  endfunction
  initial begin
    qq.push_back(qq0);
    add(qq[0]);
    $finish;
  end
endmodule
