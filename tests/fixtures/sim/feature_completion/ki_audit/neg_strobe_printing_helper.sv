// A `$strobe` helper with an output effect: the private Postponed evaluator
// refuses it, a located limit (docs/known_issues.md).
module tb;
  int x = 1;
  function int show(int v);
    $display("show %0d", v);
    return v;
  endfunction
  initial begin
    $strobe("%0d", show(x));
    #1 $finish;
  end
endmodule
