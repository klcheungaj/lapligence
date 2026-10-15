// Decision S16-D1: a production with a single rule generates that rule
// without evaluating its weight, even when the weight would be zero, and
// makes no random draw. This is llg's reading of the text below: a weight
// only chooses between alternatives.
//
// IEEE 1800-2009 18.17.1 (SystemVerilog-1800-2009.txt L31489-31492):
//   "A weight_specification shall evaluate to an integral non-negative
//   value. A weight is only meaningful when assigned to alternative
//   productions, that is, production list separated by a |. Weight
//   expressions are evaluated when their enclosing production is selected,
//   thus allowing weights to change dynamically."
module tb;
  int evaluations;
  int generated;
  function int weight(int value);
    evaluations = evaluations + 1;
    return value;
  endfunction
  initial begin
    process p;
    int unsigned after_rs, without_rs;
    p = process::self();
    p.srandom(11);
    randsequence (main)
      main : body := (weight(0)) ;
      body : { generated = generated + 1; } ;
    endsequence
    after_rs = $urandom;
    p.srandom(11);
    without_rs = $urandom;
    $display("generated=%0d evaluations=%0d draws=%0d", generated, evaluations,
             after_rs != without_rs);
    $finish;
  end
endmodule
