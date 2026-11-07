// Legal by SV 13.5.2; a task can suspend while its reference is live, and a
// queue nested in an element has no storage a reference can keep.
module tb;
  int qq[$][$];
  int qq0[$];
  task automatic late(ref int t[$]);
    #1 t.push_back(1);
  endtask
  initial begin
    qq.push_back(qq0);
    late(qq[0]);
    $finish;
  end
endmodule
