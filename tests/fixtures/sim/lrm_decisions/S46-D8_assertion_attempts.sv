// Decision S46-D8: an assertion coverage attempt of an immediate assertion
// is an executed check while the assertion is enabled; a deferred check is
// an attempt when its report matures (a flushed report is none).
//
// IEEE 1800-2009 40.5.3 (SystemVerilog-1800-2009.txt L66612): "returns the
// number of times the assertion has been attempted." 16.4.2 (L21249-21250,
// L21276): a deferred assertion flush point clears the pending reports of a
// process. 20.11 (L35048): "$assertoff shall stop the checking of all
// specified assertions until a subsequent $asserton." The text does not
// define an attempt for immediate assertions.
module tb;
  logic v = 0;
  initial begin
    $assertoff(0, tb.off);
    off: cover (1'b1);
    $asserton(0, tb.off);
    #1 $display("off %0d", $coverage_get(`SV_COV_ASSERTION, `SV_COV_MODULE, tb));
    // A glitch: dc reports the pass of v = 1, then the process resumes in
    // the same time slot with v = 0, a flush point that drops the report.
    v = 1'b1;
    #0 v = 1'b0;
    #1 $display("glitch %0d", $coverage_get(`SV_COV_ASSERTION, `SV_COV_MODULE, tb));
    v = 1'b1;
    #1 $display("matured %0d of %0d", $coverage_get(`SV_COV_ASSERTION, `SV_COV_MODULE, tb),
                $coverage_get_max(`SV_COV_ASSERTION, `SV_COV_MODULE, tb));
  end
  always @(v) dc: cover #0 (v);
endmodule
