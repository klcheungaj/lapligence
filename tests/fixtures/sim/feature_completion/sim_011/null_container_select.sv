// SV 8.4 L9653: a property container of a null handle has no storage; the
// fatal error stops the statement, and the process, before it is used.
module tb;
  class C; int q[$]; int a[string]; string s[$]; C next; endclass
  C h;
  int v;
  initial begin
    $display("before");
    h.a["k"] = 1;
    $display("not reached");
    $finish;
  end
endmodule
