// llg policy case: NULL arguments of the scope and user-data routines. The
// portable form is lrm_decisions/S41-D4_scope_routine_arguments, which leaves
// the NULL probes out because simulators diverge on the error cases.
// Decision S41-D4: argument handling of the scope and user-data routines.
// svPutUserData returns -1 for a NULL scope, NULL data or NULL key and 0
// otherwise; a second put with the same key replaces the data. svGetUserData
// returns NULL for a NULL scope or a key never stored. svGetNameFromScope and
// svGetScopeFromName return NULL for NULL. svSetScope with an argument that
// is not a scope leaves the context scope unchanged and returns it (llg also
// prints a warning on stderr).
//
// IEEE 1800-2009 H.9.3 (SystemVerilog-1800-2009.txt L72711-72714):
//   "It is illegal to pass in NULL values for either the scope or userData
//   arguments. It is also an error to call svPutUserData() with an invalid
//   svScope. This function returns -1 for all error cases, 0 upon success."
// H.9.3 (L72723-72726): svGetUserData "returns NULL for all error cases, and
//   a non-Null user data pointer upon success. This function also returns NULL
//   in the event that a prior call to svPutUserData() was never made."
// H.9.3 (L72683): svSetScope "The return is the previous active scope
//   (per svGetScope)".
// The NULL key, the NULL name arguments, replacement and an illegal
// svSetScope argument are not specified; the results here are llg choices.
//
// Build S41-D4_null_arguments.c into a shared library and load it
// with the simulator's DPI library option.
module tb;
    import "DPI-C" context function int d4_run();

    int r = 0;

    initial begin
        r = d4_run();
        $display("run=%0d", r);
        $finish;
    end
endmodule
