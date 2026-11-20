// An output process formal of a function called inside an expression: a
// located limit (docs/known_issues.md, rejected process-handle forms).
module tb;
  process q;
  function automatic int f(output process p);
    p = process::self();
    return 1;
  endfunction
  initial begin
    int x;
    x = f(q) + 1;
    $display("%0d %0d", x, q != null);
    $finish(0);
  end
endmodule
