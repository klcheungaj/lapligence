// Decision S16-D3: a rule weight that is negative or has X or Z bits is a
// run-time error. The production generates no rule, generation continues
// after it, and the run ends with exit status 1. When every weight is zero
// no rule is generated either; that only warns. The clause states the
// requirement but not what happens when it is broken (llg choice).
//
// IEEE 1800-2009 18.17.1 (SystemVerilog-1800-2009.txt L31489-31490):
//   "A weight_specification shall evaluate to an integral non-negative
//   value."
module tb;
  int minus = -1;
  logic [3:0] unknown = 4'b10x1;
  initial begin
    randsequence (main)
      main : negative zero unknown_weight { $display("after"); } ;
      negative : a := minus | b := 1 ;
      zero : a := 0 | b := 0 ;
      unknown_weight : a := unknown | b := 1 ;
      a : { $display("a"); } ;
      b : { $display("b"); } ;
    endsequence
    $display("done");
    $finish;
  end
endmodule
