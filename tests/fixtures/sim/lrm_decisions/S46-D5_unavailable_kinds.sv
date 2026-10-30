// Decision S46-D5: llg implements no statement or toggle coverage, and
// FSM state coverage (A17-D12) answers the same in a design without FSM
// pragmas, as here. For those kinds $coverage_control CHECK and START
// return SV_COV_NOCOV,
// STOP and RESET return SV_COV_OK without effect, $coverage_get and
// $coverage_get_max return 0 (SV_COV_NOCOV) and $coverage_save returns
// SV_COV_NOCOV and writes nothing.
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
  initial begin
    for (int kind = `SV_COV_FSM_STATE; kind <= `SV_COV_TOGGLE; kind++)
      $display("%0d: check %0d start %0d stop %0d reset %0d get %0d max %0d save %0d", kind,
               $coverage_control(`SV_COV_CHECK, kind, `SV_COV_HIER, $root),
               $coverage_control(`SV_COV_START, kind, `SV_COV_MODULE, tb),
               $coverage_control(`SV_COV_STOP, kind, `SV_COV_HIER, tb),
               $coverage_control(`SV_COV_RESET, kind, `SV_COV_MODULE, "tb"),
               $coverage_get(kind, `SV_COV_HIER, $root),
               $coverage_get_max(kind, `SV_COV_HIER, $root),
               $coverage_save(kind, "s46_d5_never_written.db"));
  end
endmodule
