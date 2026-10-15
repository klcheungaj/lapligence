// Decision S42-D2: vpi_chk_error returns the severity level of the error of
// the previous VPI call (vpiError = 3 for a failed lookup), not 1; calling it
// again returns the same level; the next other VPI call resets the status.
//
// IEEE 1800-2009 38.2 (SystemVerilog-1800-2009.txt L61794-61796, L61808-61809):
//   "The VPI routine vpi_chk_error() shall return an integer constant
//   representing an error severity level if the previous call to a VPI
//   routine resulted in an error. ... The error status shall be reset by any
//   VPI routine call except vpi_chk_error(). Calling vpi_chk_error() shall
//   have no effect on the error status."
// Which level a failed lookup reports is not specified; llg reports vpiError.
//
// Build S42-D2_chk_error_level.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d2()` declared as a system task).
module tb;
    logic [7:0] v = 8'd1;
    initial begin
        $d2;
        $finish;
    end
endmodule
