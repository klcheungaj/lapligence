// A call to a built-in method (`process::self()`) as a method receiver and
// operand: it has no container result (SV 9.7, 13.4.1).
class box;
  int v;
  function new(int x); v = x; endfunction
  function box twice(); box b = new(v * 2); return b; endfunction
endclass
module tb;
  function automatic box mk(int x); box b = new(x); return b; endfunction
  initial begin
    process p;
    $display("status %s", process::self().status().name());
    p = process::self();
    $display("same %0d", p == process::self());
    $display("chain %0d", mk(3).twice().twice().v);
    fork
      $display("child %s %0d", process::self().status().name(), p != process::self());
    join
    $finish;
  end
endmodule
