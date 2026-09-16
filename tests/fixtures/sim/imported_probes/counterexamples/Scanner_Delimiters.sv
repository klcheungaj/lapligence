// Static-review counterexample; NOT EXECUTED.
module tb;
  int a = -1, b = -1, n;
  initial begin
    n = $sscanf("12,34", "%d,%d", a, b);
    if (n != 2 || a != 12 || b != 34) $fatal(1, "numeric scanner consumed a delimiter");
    n = $sscanf("abc 5", "%*d %d", b);
    if (n != 0) $fatal(1, "suppressed numeric conversion was not validated");
    $finish(0);
  end
endmodule
