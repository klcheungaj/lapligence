// SIM-040 negative: an event is not a legal DPI-C formal type (35.5.6).
module tb;
    import "DPI-C" function void neg_event(input event e);
    event x;
    initial neg_event(x);
endmodule
