// Decision S46-D5: a coverage kind the model does not collect answers as
// unavailable. FSM state coverage (A17-D12) in a design without FSM
// pragmas, as here, is such a kind; statement and toggle coverage are
// available in every model whose $coverage_* calls can name them and in
// every model built for VPI applications (S46-D13, A19-D45), so they are
// never unavailable to a query. For an unavailable kind $coverage_control
// CHECK and START return SV_COV_NOCOV, STOP and RESET return SV_COV_OK
// without effect, $coverage_get and $coverage_get_max return 0
// (SV_COV_NOCOV) and $coverage_save returns SV_COV_NOCOV and writes
// nothing.
//
// IEEE 1800-2009 40.3.2.1 (SystemVerilog-1800-2009.txt L66140-66141):
// "`SV_COV_NOCOV, on a check or start operation, denotes that coverage is
// not available at any point in the specified hierarchy." L66126-66129:
// "`SV_COV_STOP stops collecting coverage information in the specified
// hierarchy. There is no effect if coverage is not being collected.
// `SV_COV_RESET resets all available coverage information in the specified
// hierarchy. There is no effect if coverage not available." Table 40-1
// lists no NOCOV result for STOP and RESET. 40.3.2.5 (L66357):
// "`SV_COV_NOCOV. No such coverage is available in this design (nothing was
// saved)."
module tb;
  initial
    $display("%0d: check %0d start %0d stop %0d reset %0d get %0d max %0d save %0d",
             `SV_COV_FSM_STATE,
             $coverage_control(`SV_COV_CHECK, `SV_COV_FSM_STATE, `SV_COV_HIER, $root),
             $coverage_control(`SV_COV_START, `SV_COV_FSM_STATE, `SV_COV_MODULE, tb),
             $coverage_control(`SV_COV_STOP, `SV_COV_FSM_STATE, `SV_COV_HIER, tb),
             $coverage_control(`SV_COV_RESET, `SV_COV_FSM_STATE, `SV_COV_MODULE, "tb"),
             $coverage_get(`SV_COV_FSM_STATE, `SV_COV_HIER, $root),
             $coverage_get_max(`SV_COV_FSM_STATE, `SV_COV_HIER, $root),
             $coverage_save(`SV_COV_FSM_STATE, "s46_d5_never_written.db"));
endmodule
