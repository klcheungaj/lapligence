// Decision S43-D4: a callback whose reason cannot occur again (a time
// callback after it ran, cbEndOfCompile, cbStartOfSimulation,
// cbEndOfSimulation) stays registered after it ran: its handle stays valid,
// vpi_get_cb_info reads its registration and vpi_remove_cb removes it (and
// returns 1). Registering an action that already occurred is accepted and
// returns a handle, but the callback never runs. Releasing a callback handle
// with vpi_release_handle keeps the callback.
//
// IEEE 1800-2009 38.39 (SystemVerilog-1800-2009.txt L65372-65374): "After
// vpi_remove_cb() is called with a handle to the callback, the handle is no
// longer valid." 38.36.2 and 38.36.3 (L64939-64962, L65007-65010) do not say
// whether a callback stays registered after its single occurrence; the text
// is silent, so the handle stays valid until it is removed.
//
// Build S43-D4_one_shot_callback_handle.c as a VPI application (llg: static
// archive given to --dpi-lib).
`timescale 1ns/1ns
module tb;
    initial #5 $finish;
endmodule
