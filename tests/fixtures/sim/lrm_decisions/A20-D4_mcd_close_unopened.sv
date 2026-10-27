// Decision A20-D4: vpi_mcd_close and vpi_mcd_flush on channels that are not
// open. vpi_mcd_close(1) returns 1 and channel 1 stays usable; closing a
// channel that is not open returns its bit; closing a descriptor of an open
// and an unopened channel closes the open one and returns the other bit;
// vpi_mcd_close(0) returns 0. Each of these sets a vpiError status. A
// successful close returns 0 with no error. vpi_mcd_flush of descriptor 0 or
// of an unopened channel returns nonzero with vpiError.
//
// IEEE 1800-2009 38.24 (SystemVerilog-1800-2009.txt L63515-63521):
//   "On success, this routine shall return a 0; on error, it shall return the
//   mcd value of the unclosed channels. ... The following descriptor is
//   predefined and cannot be closed using vpi_mcd_close(): descriptor 1 is
//   for the output channel of the tool that invoked the PLI application and
//   the current log file".
// 38.25 (L63543): vpi_mcd_flush returns "0 if successful; nonzero if
//   unsuccessful."
// The status levels and the result of descriptor 0 are llg choices.
//
// Build A20-D4_mcd_close_unopened.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d4()` declared as a system task).
module tb;
    initial begin
        $d4;
        $finish;
    end
endmodule
