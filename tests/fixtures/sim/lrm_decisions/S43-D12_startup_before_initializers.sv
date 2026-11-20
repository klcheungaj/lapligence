// Decision S43-D12: a VPI application starts before static initialization.
// The startup routines run first and register the user system functions,
// then the compiletf of every call site runs, then the static declaration
// initializers run (package variables before the design), and a call of a
// user system function in an initializer runs its calltf there, with access
// to the design. cbEndOfCompile and cbStartOfSimulation run after the
// initializers and read the initialized values; the procedures start last
// (industry-standard behaviour).
//
// IEEE 1800-2009 36.9.1 (SystemVerilog-1800-2009.txt L56584):
//   "The registration of system tasks shall occur prior to elaboration or the
//   resolution of references."
// IEEE 1800-2009 36.10.2 (L56682-56684):
//   "The next earliest phase is when the sizetf routines are called for the
//   user-defined system functions. At this phase, no additional access is
//   permitted. After the sizetf routines are called, the routines registered
//   for reason cbEndOfCompile are called."
// IEEE 1800-2009 6.8 (L5286-5287):
//   "Setting the initial value of a static variable as part of the variable
//   declaration (including static class members) shall occur before any
//   initial or always procedures are started"
// IEEE 1800-2009 38.36.3 (L65008):
//   "cbStartOfSimulation Start of simulation (beginning of time 0 simulation
//   cycle)"
// The text does not place static initialization against cbEndOfCompile; it
// only requires registration before the references resolve and the
// initializers before the procedures.
//
// Build S43-D12_startup_before_initializers.c as a VPI application (llg:
// static archive given to --dpi-lib).
`timescale 1ns/1ns
package pkg;
    int p = $order(1);
endpackage
module tb;
    int k = 42;
    int y = $order(2);
    initial begin
        $display("initial k=%0d y=%0d p=%0d", k, y, pkg::p);
        $finish;
    end
endmodule
