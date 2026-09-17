// Static-review counterexample; NOT EXECUTED.
module tb;
  logic driver = 0;
  wire #2 original;
  wire mirror;
  bit seen;
  assign original = driver;
  alias original = mirror;
  initial begin #4; driver = 1; end
  initial begin wait (mirror === 1'b1); seen = 1; end
  initial begin
    #8;
    // Do not poll mirror before checking seen: alias reads currently refresh it.
    if (!seen) $fatal(1, "delayed alias publication did not notify the waiter");
    $finish(0);
  end
endmodule
