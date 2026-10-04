// Negative (SV 13.3.2): an automatic variable cannot be referenced in the
// intra-assignment event control of a nonblocking assignment.
module tb;
  int q, seen = 0;
  function int f(input int v);
    seen++;
    return v;
  endfunction
  initial begin
    for (int i = 0; i < 2; i++) q <= @(f(i)) 1;
    #1 $finish(0);
  end
endmodule
