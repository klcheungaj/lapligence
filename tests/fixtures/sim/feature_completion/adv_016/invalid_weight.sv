// Run-time check of IEEE 1800-2009 18.17.1 inside a recursive production
// (decision S16-D3): a negative rule weight ends the run at once; nothing
// after it runs, final procedures do not run and the exit status is 1.
module tb;
  int minus = -2;
  string trace;
  final $display("final %s", trace);
  initial begin
    randsequence (top)
      top : walk(2) { $display("walk %s", trace); } ;
      walk (int k) : if (k > 0) step(k) ;
      step (int k) : choose(k) x walk(k - 1) ;
      choose (int k) : x := (k == 1 ? minus : 0) | y := 0 ;
      x : { trace = {trace, "x"}; $display("trace %s", trace); } ;
      y : { trace = {trace, "y"}; } ;
    endsequence
    $display("done");
    $finish;
  end
endmodule
