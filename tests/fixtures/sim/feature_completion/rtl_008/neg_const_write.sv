// SV2009 6.20.6: a `const` variable keeps its initialized value; a later
// procedural write is illegal.
module tb;
  const int c = 5;
  initial begin
    c = 6;
    $finish;
  end
endmodule
