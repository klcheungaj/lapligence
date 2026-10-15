// Decision S46-D12: every assertion item collects coverage, so
// $coverage_control CHECK and START of assertion coverage return SV_COV_OK
// when the selected hierarchy holds an assertion item and SV_COV_NOCOV
// when it holds none; they never return SV_COV_PARTIAL, also for a
// hierarchy whose other instances have no assertion.
//
// IEEE 1800-2009 40.3.2.1 (SystemVerilog-1800-2009.txt L66135-66136):
// "`SV_COV_OK, on a check operation, denotes that coverage is fully
// available in the specified hierarchy." L66142-66143: "`SV_COV_PARTIAL,
// on a check or start operation, denotes that coverage is only partially
// available in the specified hierarchy." Whether an instance without
// assertions makes assertion coverage partial is not stated.
module checked(input logic clk);
  ap: assert property (@(posedge clk) 1'b1);
endmodule

module plain;
endmodule

module tb;
  logic clk = 0;
  checked c(clk);
  plain p();
  initial
    $display("tb %0d %0d c %0d p %0d %0d",
             $coverage_control(`SV_COV_CHECK, `SV_COV_ASSERTION, `SV_COV_HIER, tb),
             $coverage_control(`SV_COV_START, `SV_COV_ASSERTION, `SV_COV_HIER, tb),
             $coverage_control(`SV_COV_CHECK, `SV_COV_ASSERTION, `SV_COV_MODULE, c),
             $coverage_control(`SV_COV_CHECK, `SV_COV_ASSERTION, `SV_COV_MODULE, p),
             $coverage_control(`SV_COV_START, `SV_COV_ASSERTION, `SV_COV_HIER, p));
endmodule
