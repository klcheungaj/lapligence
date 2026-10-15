// Decision S46-D10: vpi_control with vpiCoverageStart/Stop/Reset/Check
// returns the $coverage_control status (SV_COV_OK 1, SV_COV_NOCOV 0,
// SV_COV_ERROR -1); an instance handle selects that instance's own
// assertions (as SV_COV_MODULE), an assertion handle that assertion.
// vpi_get(vpiAssertCoverage, instance) counts the instance's own assertions
// with a nonvacuous success, as $coverage_get does (S46-D4), while
// vpiCovered of an assertion also requires that it never failed.
//
// IEEE 1800-2009 40.5.4 (SystemVerilog-1800-2009.txt L66756-66757,
// L66770-66771): "vpi_control(<coverageControl>, <coverageType>,
// instance_handle) / vpi_control(<coverageControl>, <coverageType>,
// assertion_handle) ... The semantics and behavior are per the
// $coverage_control() system function". 40.5.3 (L66585-66587):
// "vpi_get(<coverageType>, instance_handle) returns the number of covered
// items of the given coverage type in the given instance." L66606-66607:
// "For assertions, vpiCovered implies that the assertion has been
// attempted, has succeeded at least once, and has never failed." The return
// value of the coverage operations and the hierarchy an instance handle
// selects are not stated.
module sub(input logic clk, input logic a);
  ok: assert property (@(posedge clk) a) else $display("ok failed");
endmodule

module tb;
  logic clk = 0, a = 1;
  sub s(clk, a);
  always #5 clk = ~clk;
  top: assert property (@(posedge clk) 1'b1);
  initial begin
    $d10(0);
    #6 a = 1'b0;
    #10 $d10(1);
    $finish;
  end
endmodule
