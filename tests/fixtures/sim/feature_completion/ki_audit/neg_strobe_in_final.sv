// A `$strobe` in a final block: no Postponed region runs after finals, so it
// is a located limit (docs/known_issues.md).
module tb;
  int x = 1;
  initial #1 $finish;
  final $strobe("%0d", x);
endmodule
