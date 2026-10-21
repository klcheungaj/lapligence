// Decision S41-D4: user data stored in a scope. svPutUserData returns 0 on
// success; a second put with the same key replaces the data. svGetUserData
// returns the stored pointer, and NULL for a key never stored.
//
// IEEE 1800-2009 H.9.3 (SystemVerilog-1800-2009.txt L72711-72714):
//   "It is illegal to pass in NULL values for either the scope or userData
//   arguments. It is also an error to call svPutUserData() with an invalid
//   svScope. This function returns -1 for all error cases, 0 upon success."
// H.9.3 (L72723-72726): svGetUserData "returns NULL for all error cases, and
//   a non-Null user data pointer upon success. This function also returns NULL
//   in the event that a prior call to svPutUserData() was never made."
// Replacement is not specified; the result here is an llg choice. The
// routines' NULL arguments are not exercised here, since simulators diverge
// on the error cases the text calls illegal; llg's results for them are
// checked by llg_policy/S41-D4_null_arguments.
//
// Build S41-D4_scope_routine_arguments.c into a shared library and load it
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
