// Decision A20-D9: cbForce and cbRelease. A record runs right after each
// force or release statement took effect, before the statement's process
// continues, with obj a vpiForce or vpiRelease handle that is valid only
// while the callback runs (vpiLhs gives the forced object) and the value of
// the left-hand side after the statement. A record placed on an object runs
// for each statement that writes any of its bits. vpi_put_value with
// vpiForceFlag or vpiReleaseFlag, and the reevaluation of a force whose
// right-hand side changed, are not statements and run no callback.
// Registering cbAssign or cbDeassign succeeds; llg rejects procedural assign
// and deassign statements, so those records never run.
//
// IEEE 1800-2009 38.36.1 (SystemVerilog-1800-2009.txt L64646-L64647):
// "cbForce/cbRelease After a force or release has occurred. cbAssign/
// cbDeassign After a procedural assign or deassign statement has been
// executed."; L64695-L64697: "For force and release callbacks, if this is
// set to NULL, every force and release shall generate a callback.";
// L64780-L64783: "For cbForce, cbRelease, cbAssign, and cbDeassign callbacks,
// the object returned in the obj field shall be a handle to the force,
// release, assign, or deassign statement. The value field shall contain the
// resultant value of the left-hand expression. In the case of a release, the
// value field shall contain the value after the release has occurred.";
// L64788-L64789: "It is illegal to attempt to place a callback for reason
// cbForce, cbRelease, or cbDisable on a variable bit-select." When the
// callback runs relative to the process, which statements a record on an
// object sees, and VPI forces are llg choices.
//
// Build A20-D9_force_release_callbacks.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d9()` and `task $d9_vpi()` declared as
// system tasks).
`timescale 1ns/1ns
module tb;
    logic [7:0] v, src;
    wire [7:0] n;
    assign n = src;
    initial begin
        v = 8'h11;
        src = 8'h22;
        $d9;
        #1 force v = 8'h5a;
        $display("after force at %0t", $time);
        #1 force n[3:0] = 4'h3;
        #1 release n;
        #1 release v;
        #1 force v = src;
        #1 src = 8'h33;
        #1 release v;
        #1 $d9_vpi;
        #1 $finish;
    end
endmodule
