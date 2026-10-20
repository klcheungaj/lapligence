// Run-time checks of IEEE 1800-2009 18.17.4 and 18.17.5 inside recursive
// productions and with a NaN join ratio (decisions S16-D4 and S16-D7): each
// reports a located error, generation continues and the run ends with exit
// status 1. All-zero weights generate nothing and only warn (S16-D3); a
// negative weight ends the run (invalid_weight.sv).
module tb;
  int minus = -2;
  real zero = 0.0;
  string trace;
  initial begin
    randsequence (top)
      top : walk(2) { $display("walk %s", trace); } joined { $display("length %0d", trace.len()); } ;
      walk (int k) : if (k > 0) step(k) ;
      step (int k) : choose repeat (k == 1 ? minus : 1) x walk(k - 1) ;
      choose : x := 0 | y := 0 ;
      joined : rand join (zero / zero) x y ;
      x : { trace = {trace, "x"}; } ;
      y : { trace = {trace, "y"}; } ;
    endsequence
    $display("done");
    $finish;
  end
endmodule
