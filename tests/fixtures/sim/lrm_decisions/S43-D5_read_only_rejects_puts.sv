// Decision S43-D5: in cbReadOnlySynch (and every read-only region) each
// vpi_put_value to a design object fails with a vpiError status and changes
// nothing: no-delay, delayed (even to a later time slot) and force. A
// zero-delay cbReadWriteSynch cannot be placed there; one with a delay can.
//
// IEEE 1800-2009 38.36.2 (SystemVerilog-1800-2009.txt L64947-64948):
// cbReadOnlySynch "shall occur the same as for cbReadWriteSynch, except that
// writing values or scheduling events before the next scheduled event is not
// allowed." L64979 makes the zero-delay cbReadWriteSynch an error. 4.4.2.9
// (L3212-3214) forbids writes in the Postponed region. The text does not
// say how a forbidden put is reported, nor whether a delayed put past the
// next scheduled event is allowed; llg rejects every put with an error.
//
// Build S43-D5_read_only_rejects_puts.c as a VPI application (llg: static
// archive given to --dpi-lib).
`timescale 1ns/1ns
module tb;
    logic [3:0] d = 4'd1;
    always @(d) $display("d=%0d at %0t", d, $time);
    initial #20 $finish;
endmodule
