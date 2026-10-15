// Unbounded recursion stops at llg's limit of 10000 active generations of
// recursive productions with a located fatal run-time error (exit status 1),
// instead of exhausting memory or the C stack.
module tb;
  int reached;
  initial begin
    $display("start");
    randsequence (top)
      top : forever_down(0) ;
      forever_down (int n) : { reached = n; } forever_down(n + 1) ;
    endsequence
    $display("never");
    $finish;
  end
  final $display("reached %0d", reached);
endmodule
