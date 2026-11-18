// An event handle other than a declared named event (a class event property)
// in the same list as a process-evaluated helper with a visible write: a
// located limit (docs/known_issues.md, effectful helpers in runtime-callback
// evaluators).
module tb;
  class C; event ev; endclass
  C c;
  int seen = 0;
  int x = 0;
  function int f(input int v);
    seen++;
    return v;
  endfunction
  initial begin
    c = new;
    fork
      begin @(c.ev or f(x)); $display("woke %0d", seen); end
      #1 ->c.ev;
    join
    $finish;
  end
endmodule
