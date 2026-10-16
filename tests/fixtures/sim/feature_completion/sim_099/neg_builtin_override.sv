// IEEE 1800-2009 36.3.2: builtin_override.c registers $random, which this
// design calls; llg stops at startup instead of running its built-in.
module tb;
  integer r;
  initial begin
    r = $random;
    $display("random %0d", r);
    $finish;
  end
endmodule
