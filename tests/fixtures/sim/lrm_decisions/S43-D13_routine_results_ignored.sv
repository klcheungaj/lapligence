// Decision S43-D13: the value a compiletf, calltf or callback routine
// returns is ignored, and an error in a VPI routine only sets the status
// that vpi_chk_error reads. Neither stops the run or changes its exit
// status: here the compiletf, the calltf, a cbStartOfSimulation callback and
// a cbAfterDelay callback each return 1, the calltf makes vpi_put_value fail
// with vpiError (level 3), and the run goes on to $finish (industry-standard
// behaviour).
//
// IEEE 1800-2009 36.8.2 (SystemVerilog-1800-2009.txt L56526-56528):
//   "A compiletf VPI application routine shall be called when the
//   user-defined system task or system function name is encountered during
//   parsing or compiling the SystemVerilog source code. This routine is
//   typically used to check the correctness of any arguments"
// IEEE 1800-2009 38.36 (L64618-64619):
//   "The cb_rtn field of the s_cb_data structure shall be set to the
//   application routine, which shall be invoked when the simulator executes
//   the callback."
// IEEE 1800-2009 36.10.1 (L56658-56660):
//   "To determine whether an error occurred, the routine vpi_chk_error() (see
//   38.2) shall be provided. The vpi_chk_ error() routine shall return a
//   nonzero value if an error occurred in the previously called VPI
//   routine."
// The text gives the return values of these routines no meaning.
//
// Build S43-D13_routine_results_ignored.c as a VPI application (llg: static
// archive given to --dpi-lib).
`timescale 1ns/1ns
module tb;
    initial begin
        $display("before");
        $ret1;
        $display("after");
        #2 $display("at %0t", $time);
        $finish;
    end
    final $display("final");
endmodule
