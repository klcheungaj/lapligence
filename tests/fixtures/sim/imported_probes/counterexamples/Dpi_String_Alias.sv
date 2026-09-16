// Static-review counterexample; NOT EXECUTED.
module tb;
  import "DPI-C" function string review_echo(inout string s);
  string value, result;
  initial begin
    value = "retained bytes";
    result = review_echo(value);
    if (result != "retained bytes" || value != "retained bytes")
      $fatal(1, "aliased DPI string result was not retained before copy-out");
    $finish(0);
  end
endmodule
