// Unsupported boundary: a named event cannot share an event control with a process-evaluated helper.
module tb;
  event ev;
  int a = 0, seen = 0;
  function int f(input int x); seen++; return x; endfunction
  initial begin
    fork
      begin @(ev or f(a)); $display("woke"); end
      #1 -> ev;
    join
    $finish(0);
  end
endmodule
