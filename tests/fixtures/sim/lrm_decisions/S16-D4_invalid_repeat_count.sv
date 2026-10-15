// Decision S16-D4: a repeat production whose count is negative or has X or Z
// bits is a run-time error. Its production is not generated, generation
// continues after it, and the run ends with exit status 1. The clause states
// the requirement but not what happens when it is broken (llg choice).
//
// IEEE 1800-2009 18.17.4 (SystemVerilog-1800-2009.txt L31599-31600):
//   "The repeat expression shall evaluate to a non-negative integral value.
//   That value specifies the number of times that the corresponding
//   production is generated."
module tb;
  int minus = -1;
  logic [3:0] unknown = 4'b1z01;
  initial begin
    randsequence (main)
      main : repeat (minus) a repeat (unknown) a repeat (2) b
             { $display("after"); } ;
      a : { $display("a"); } ;
      b : { $display("b"); } ;
    endsequence
    $display("done");
    $finish;
  end
endmodule
