// Decision S11-D4: an actual that must be built (a container call result) is
// built where its call is evaluated, so a short-circuited operand builds
// nothing.
// IEEE 1800-2009 11.3.5 (SystemVerilog-1800-2009.txt L14434-14435): "When short
// circuiting occurs, any side effects or runtime errors that would have
// occurred due to evaluation of the short circuited operand expression shall
// not occur."
module tb;
  int q[$];
  int n = 0;
  typedef int ia_t[$];
  function automatic ia_t twice(int a[$]);
    n++;
    return {a, a};
  endfunction
  function automatic int total(int a[$]);
    return a.sum();
  endfunction
  initial begin
    q.push_back(1);
    if (q.size() > 100 && total(twice(q)) > 0) $display("no");
    $display("%0d", n);
    if (q.size() == 1 && total(twice(q)) > 0) $display("yes");
    $display("%0d", n);
    $finish;
  end
endmodule
