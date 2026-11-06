// A wait re-evaluates its condition where a handle local of the statement is
// gone, so a computed receiver is rejected explicitly.
module tb;
  class C;
    int q[$];
  endclass
  C list[$];
  initial begin
    list.push_back(C'(null));
    wait (list[0].q.size() > 0);
  end
endmodule
