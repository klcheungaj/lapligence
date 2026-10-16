// IEEE 1800-2009 36.3.2: builtin_override.c registers $random, which this
// design does not call, so no built-in is overridden and the run proceeds.
module tb;
  initial begin
    $display("no built-in overridden");
    $finish;
  end
endmodule
