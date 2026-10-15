// Decision S16-D7: a rand join expression outside 0.0 to 1.0, or one that is
// not a number, is a run-time error. The join then uses the default 0.5,
// generation continues, and the run ends with exit status 1. The clause
// states the requirement but not what happens when it is broken (llg
// choice).
//
// IEEE 1800-2009 18.17.5 (SystemVerilog-1800-2009.txt L31655):
//   "The optional expression following the rand join keywords shall be a
//   real number in the range of 0.0 to 1.0."
module tb;
  real ratio = 2.0;
  string trace;
  initial begin
    randsequence (top)
      top : joined { $display("length=%0d", trace.len()); } ;
      joined : rand join (ratio) s1 s2 ;
      s1 : a b ;
      s2 : a b ;
      a : { trace = {trace, "a"}; } ;
      b : { trace = {trace, "b"}; } ;
    endsequence
    $display("done");
    $finish;
  end
endmodule
