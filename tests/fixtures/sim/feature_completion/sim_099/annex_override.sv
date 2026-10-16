// IEEE 1800-2009 36.3.2: a PLI application registered under the name of a
// built-in llg does not implement (the informative Annex D services
// $reset_count, D.8, and $list, D.6) provides it. annex_override.c returns 2
// from $reset_count and prints a line from $list.
module tb;
  integer resets;
  initial begin
    resets = $reset_count;
    $display("reset count %0d", resets);
    $list;
    $display("done");
    $finish;
  end
endmodule
