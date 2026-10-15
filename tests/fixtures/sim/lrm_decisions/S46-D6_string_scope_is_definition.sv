// Decision S46-D6: a string hierarchy argument of $coverage_control,
// $coverage_get_max and $coverage_get names a module definition; a string
// that names no definition of the design, such as an instance name, is a bad
// argument (SV_COV_ERROR).
//
// IEEE 1800-2009 40.3.2.1 (SystemVerilog-1800-2009.txt L66203-66205):
// "NOTE-Definition names are represented as strings, whereas instance
// names are referenced by hierarchical paths." L66138-66139: "`SV_COV_ERROR,
// on all operations, means that the control operation failed without any
// effect, typically due to errors in arguments, such as a nonexisting
// module."
module sub(input logic clk);
  ap: assert property (@(posedge clk) 1'b1);
endmodule

module tb;
  logic clk = 0;
  sub u1(clk);
  sub u2(clk);
  initial
    $display("sub %0d u1 %0d instance %0d",
             $coverage_get_max(`SV_COV_ASSERTION, `SV_COV_MODULE, "sub"),
             $coverage_get_max(`SV_COV_ASSERTION, `SV_COV_MODULE, "u1"),
             $coverage_get_max(`SV_COV_ASSERTION, `SV_COV_MODULE, u1));
endmodule
